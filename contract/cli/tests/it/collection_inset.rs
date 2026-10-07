//! LLP 1010 §6.9: a virtualized list's main-axis padding is CSS's room
//! before the first row and after the last, inside the scroll content. The
//! host reports offsets from the first row's start; the runner reads the
//! padding after the last row from the list's layout, so its end (the
//! window, `reachend`, a followed end, `scroll-start: end`, a clamped
//! `scrollIntoView`) is the true one.
use exact_kernel::{Env, Kernel, Offer};
use exact_plan::Value;
use exact_runner::{
    CollectionFeedback, CollectionSnapshot, DataError, DataSource, RowMeasurement, Runner,
};

const APP: &str = r#"component App
  state count = 100
  state ends = 0
  resource rows = rows(count) as shape list<number>
  action more(n: number)
    count = count + n
  action onEnd
    ends = ends + 1
  action go(n: number)
    scrollIntoView("feed", n, block="start")
  view
    column testId="root"
      list id="feed" virtualized=true height=600 width=400 overflow-x="hidden" estimated-item-height=100 padding-top=92 padding-bottom="calc(env(safe-area-inset-bottom) + 49px)" reachend=onEnd
        each x in rows key=x
          text `${x}` height=100 width="100%"
"#;
/// The padding before the first row and after the last (49 + a 34-point
/// home indicator), the port, and every row's size.
const TOP: f64 = 92.0;
const END: f64 = 83.0;
const PORT: f64 = 600.0;
const ROW: f64 = 100.0;

struct Data;
impl DataSource for Data {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        let n = args[0].as_number().unwrap() as usize;
        Ok(Value::list(
            (0..n).map(|i| Value::Number(i as f64)).collect(),
        ))
    }
}
fn boot(source: &str) -> Runner<Data> {
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.kernel_mut()
        .set_env(Env::new(47.0, 0.0, 34.0, 0.0))
        .unwrap();
    layout(&mut r);
    r
}
fn layout(r: &mut Runner<Data>) {
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
}
fn list(r: &Runner<Data>) -> CollectionSnapshot {
    r.collections().into_iter().next().unwrap()
}
/// The farthest a padded scroller's `scrollTop` goes, under a padding after
/// the rows of `end`.
fn max_top_with(c: &CollectionSnapshot, end: f64) -> f64 {
    (TOP + c.total_extent + end - PORT).max(0.0)
}
fn max_top(c: &CollectionSnapshot) -> f64 {
    max_top_with(c, END)
}
/// The padding after the rows as laid out now.
fn end_of(r: &Runner<Data>) -> f64 {
    let k = r.kernel();
    let node = k.node(list(r).view).unwrap();
    k.resolved_padding(node.key).unwrap().3 as f64
}
/// A host: its `scrollTop` (`top`) counts from the padding's top, as a
/// browser's does; it reports the offset from the first row's start
/// (negative in the padding before it), moves
/// where a correction says (plus the padding), clamped to its range, and
/// measures every mounted row, until the list asks for nothing more. The
/// `scrollTop` it ends at.
fn host(r: &mut Runner<Data>, mut top: f64) -> f64 {
    let mut sequence = 0;
    for _ in 0..12 {
        layout(r);
        let c = list(r);
        if let Some(correction) = c.correction {
            top = correction.offset + TOP;
        }
        // A scroll view keeps its offset inside its range.
        top = top.clamp(0.0, max_top_with(&c, end_of(r)));
        sequence = sequence.max(c.scroll_sequence) + 1;
        let changed = r
            .collection_feedback(CollectionFeedback {
                view: c.view,
                revision: c.revision,
                scroll_sequence: sequence,
                offset: top - TOP,
                port_main: PORT,
                port_cross: 400.0,
                cross: 400.0,
                measurements: c
                    .rows
                    .iter()
                    .map(|row| RowMeasurement {
                        view: row.view,
                        epoch: row.epoch,
                        size: ROW,
                    })
                    .collect(),
                focus_view: None,
                interaction_view: None,
            })
            .unwrap();
        if list(r).correction.is_none() && changed.receipts.is_empty() {
            break;
        }
    }
    top
}

#[test]
fn row_zero_sits_below_the_padding_and_the_window_starts_there() {
    let mut r = boot(APP);
    let top = host(&mut r, 0.0);
    assert_eq!(top, 0.0, "the padding shows at scrollTop 0");
    let c = list(&r);
    let k = r.kernel();
    let first = c
        .rows
        .iter()
        .find(|row| row.index == 0)
        .expect("row 0 mounted");
    let list_node = k.node(c.view).unwrap();
    assert_eq!(
        k.resolved_padding(list_node.key).map(|(_, t, _, b)| (t, b)),
        Some((TOP as f32, END as f32)),
        "env() resolved in the padding after the rows"
    );
    assert_eq!(
        k.node(first.view).unwrap().frame.y,
        TOP as f32,
        "row 0 starts below padding-top"
    );
    assert_eq!(r.slot("ends").unwrap().as_number(), Some(0.0));
}

/// At the true end the port is past the rows by the padding after them; the
/// runner keeps it there (it once pulled it back to the rows' end) and the
/// last row is in the window, so `reachend` fires.
#[test]
fn the_true_end_is_the_rows_and_the_padding_after_them() {
    let mut r = boot(APP);
    host(&mut r, 0.0);
    let end = max_top(&list(&r));
    assert_eq!(end, TOP + 100.0 * ROW + END - PORT);
    let top = host(&mut r, end);
    assert_eq!(
        top, end,
        "no correction pulls the port back to the rows' end"
    );
    let c = list(&r);
    assert!(c.correction.is_none(), "{:?}", c.correction);
    assert!(
        c.rows.iter().any(|row| row.index == 99),
        "the last row is mounted"
    );
    assert_eq!(
        r.slot("ends").unwrap().as_number(),
        Some(1.0),
        "reachend at the end"
    );
}

/// A list that follows its end follows the true one: rows appended at the
/// end land with the padding after them still showing.
#[test]
fn a_followed_end_keeps_the_padding_after_the_last_row() {
    let mut r = boot(&APP.replace("reachend=onEnd", "scrollFollowEnd=true"));
    host(&mut r, 0.0);
    let end = max_top(&list(&r));
    host(&mut r, end);
    r.act("more", vec![Value::Number(5.0)]).unwrap();
    let c = list(&r);
    assert_eq!(c.count, 105);
    let top = host(&mut r, end);
    let c = list(&r);
    assert_eq!(c.total_extent, 105.0 * ROW);
    assert_eq!(top, max_top(&c), "following the true end");
}

/// `scroll-start: end` opens at the true end, and stays.
#[test]
fn scroll_start_end_opens_with_the_end_padding_showing() {
    let mut r = boot(&APP.replace("reachend=onEnd", "scroll-start=\"end\""));
    let top = host(&mut r, 0.0);
    let c = list(&r);
    assert_eq!(top, max_top(&c), "opened at the true end");
    assert!(c.rows.iter().any(|row| row.index == 99));
}

/// `scrollIntoView(block="start")` puts a row at the scroller's top edge, as a
/// padded CSS scroller does: `scrollTop` is the row's start plus the
/// padding. The last row clamps at the true end, not the rows' end.
#[test]
fn scroll_into_view_start_puts_the_row_at_the_top_edge() {
    let mut r = boot(APP);
    host(&mut r, 0.0);
    r.act("go", vec![Value::Number(50.0)]).unwrap();
    let top = host(&mut r, 0.0);
    assert_eq!(top, TOP + 50.0 * ROW);
    r.act("go", vec![Value::Number(99.0)]).unwrap();
    let top = host(&mut r, top);
    assert_eq!(top, max_top(&list(&r)), "clamped at the true end");
    assert!(
        exact_runner::agent::state(&r).contains("\"status\":\"done\""),
        "the request settled"
    );
}

/// The padding after the rows changes under a followed end (a rotation's
/// safe area): grown, the host's port is where the old end was and the list
/// moves it to the new one; shrunk, the host clamps it to the new end, and
/// the list still follows.
#[test]
fn a_followed_end_follows_the_padding_as_it_changes() {
    let mut r = boot(&APP.replace("reachend=onEnd", "scrollFollowEnd=true"));
    host(&mut r, 0.0);
    let end = max_top(&list(&r));
    host(&mut r, end);
    r.kernel_mut()
        .set_env(Env::new(47.0, 0.0, 60.0, 0.0))
        .unwrap();
    let top = host(&mut r, end);
    assert_eq!(end_of(&r), END + 26.0);
    assert_eq!(
        top,
        max_top_with(&list(&r), END + 26.0),
        "moved to the grown end"
    );
    r.kernel_mut()
        .set_env(Env::new(47.0, 0.0, 0.0, 0.0))
        .unwrap();
    let top = host(&mut r, top);
    assert_eq!(
        top,
        max_top_with(&list(&r), 49.0),
        "clamped to the shrunk end"
    );
    r.act("more", vec![Value::Number(5.0)]).unwrap();
    let top = host(&mut r, top);
    assert_eq!(top, max_top_with(&list(&r), 49.0), "and still following it");
}

/// `scroll-padding` (LLP 1010 §6.9): `scrollIntoView` aligns a row within
/// the port less its scroll padding, as CSS aligns in the snapport. With
/// `scroll-padding-top` the header's height, the first row's `start` is
/// `scrollTop` 0 (Bluesky's soft reset under its header).
const SNAP: &str = r#"component App
  state count = 100
  resource rows = rows(count) as shape list<number>
  action start(n: number)
    scrollIntoView("feed", n, block="start")
  action center(n: number)
    scrollIntoView("feed", n, block="center")
  action end(n: number)
    scrollIntoView("feed", n, block="end")
  action nearest(n: number)
    scrollIntoView("feed", n, block="nearest")
  view
    column testId="root"
      list id="feed" virtualized=true height=600 width=400 overflow-x="hidden" estimated-item-height=100 padding-top=92 padding-bottom="calc(env(safe-area-inset-bottom) + 49px)" scroll-padding="92 0 calc(env(safe-area-inset-bottom) + 49px)"
        each x in rows key=x
          text `${x}` height=100 width="100%"
"#;
fn into(r: &mut Runner<Data>, action: &str, row: f64, from: f64) -> f64 {
    r.act(action, vec![Value::Number(row)]).unwrap();
    let top = host(r, from);
    assert!(
        exact_runner::agent::state(r).contains("\"status\":\"done\""),
        "{action} {row} settled: {}",
        exact_runner::agent::state(r)
    );
    top
}

#[test]
fn scroll_into_view_aligns_within_the_scroll_padding() {
    let mut r = boot(SNAP);
    host(&mut r, 0.0);
    // The snapport is the port less 92 at the top and 83 at the bottom.
    let (low, high) = (TOP, PORT - END);
    let top = into(&mut r, "start", 50.0, 0.0);
    assert_eq!(top, TOP + 50.0 * ROW - low, "below the header");
    let top = into(&mut r, "end", 50.0, top);
    assert_eq!(top, TOP + 51.0 * ROW - high, "above the tab bar");
    let top = into(&mut r, "center", 50.0, top);
    assert_eq!(
        top,
        TOP + 50.5 * ROW - (low + high) / 2.0,
        "centred in the snapport"
    );
    // The last row's end clamps at the true end.
    let top = into(&mut r, "end", 99.0, top);
    assert_eq!(top, max_top(&list(&r)));
    // Nearest: a row under the header comes down to just below it.
    let top = into(&mut r, "nearest", 99.0 - 5.0, top);
    assert_eq!(top, TOP + 94.0 * ROW - low);
    // The first row's start is `scrollTop` 0, from far away; its end too
    // clamps there, at the padding before it.
    let top = into(&mut r, "start", 0.0, top);
    assert_eq!(top, 0.0, "the soft reset: the very top");
    let top = into(&mut r, "start", 50.0, top);
    assert_eq!(top, TOP + 50.0 * ROW - low);
    let top = into(&mut r, "end", 0.0, top);
    assert_eq!(top, 0.0);
}

/// Without scroll padding a row's start is the port's top edge, as before,
/// and the first row's is the padding's height down.
#[test]
fn without_scroll_padding_start_is_the_ports_top_edge() {
    let mut r = boot(&SNAP.replace(
        " scroll-padding=\"92 0 calc(env(safe-area-inset-bottom) + 49px)\"",
        "",
    ));
    host(&mut r, 0.0);
    let top = into(&mut r, "start", 0.0, 4000.0);
    assert_eq!(top, TOP);
    let top = into(&mut r, "center", 1.0, top);
    assert_eq!(top, 0.0, "row 1's centre is above the port's: the very top");
}

/// A request before the list's first report, when the runner knows no
/// padding yet, still lands at the very top once a report brings it (Grok's
/// first scroll-padding review: a report of 0 for the whole padding finished
/// it a padding's height down).
#[test]
fn the_soft_reset_before_any_report_still_reaches_the_top() {
    let mut r = boot(SNAP);
    r.act("start", vec![Value::Number(0.0)]).unwrap();
    let top = host(&mut r, 0.0);
    assert_eq!(top, 0.0);
    assert!(exact_runner::agent::state(&r).contains("\"status\":\"done\""));
}

/// `nearest` at `scrollTop` 0: a row inside the snapport stays, and the port
/// stays at the very top (it once moved down to the first row).
#[test]
fn nearest_at_the_top_leaves_the_port_there() {
    let mut r = boot(SNAP);
    host(&mut r, 0.0);
    let top = into(&mut r, "nearest", 1.0, 0.0);
    assert_eq!(top, 0.0);
}

/// `nearest` on a row taller than the snapport that covers it stays, as
/// CSSOM View says; one partly above aligns its end, the nearer edge.
#[test]
fn nearest_keeps_a_row_that_covers_the_snapport() {
    let mut r = boot(&SNAP.replace(
        "text `${x}` height=100",
        "text `${x}` height=(x == 3 ? 700 : 100)",
    ));
    let tall = |i: usize| if i == 3 { 700.0 } else { ROW };
    // A host that measures row 3 at 700.
    let mut host_tall = |r: &mut Runner<Data>, mut top: f64| {
        let mut sequence = 0;
        for _ in 0..12 {
            layout(r);
            let c = list(r);
            if let Some(correction) = c.correction {
                top = correction.offset + TOP;
            }
            top = top.clamp(0.0, max_top_with(&c, end_of(r)));
            sequence = sequence.max(c.scroll_sequence) + 1;
            let changed = r
                .collection_feedback(CollectionFeedback {
                    view: c.view,
                    revision: c.revision,
                    scroll_sequence: sequence,
                    offset: top - TOP,
                    port_main: PORT,
                    port_cross: 400.0,
                    cross: 400.0,
                    measurements: c
                        .rows
                        .iter()
                        .map(|row| RowMeasurement {
                            view: row.view,
                            epoch: row.epoch,
                            size: tall(row.index),
                        })
                        .collect(),
                    focus_view: None,
                    interaction_view: None,
                })
                .unwrap();
            if list(r).correction.is_none() && changed.receipts.is_empty() {
                break;
            }
        }
        top
    };
    host_tall(&mut r, 0.0);
    // Row 3 runs 300..1000 in rows; at scrollTop 92 + 250 the snapport
    // (342..767 in rows) is inside it.
    let at = TOP + 250.0;
    host_tall(&mut r, at);
    r.act("nearest", vec![Value::Number(3.0)]).unwrap();
    assert_eq!(host_tall(&mut r, at), at, "covering: stays");
    // From the top its start is inside the snapport and its end below it:
    // a row taller than the snapport aligns its start.
    host_tall(&mut r, 0.0);
    r.act("nearest", vec![Value::Number(3.0)]).unwrap();
    assert_eq!(host_tall(&mut r, 0.0), TOP + 300.0 - TOP);
}
