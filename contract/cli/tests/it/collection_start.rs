//! LLP 1010 §6.5: `scroll-start="end"` opens a virtualized list at its end —
//! a transcript — on measured sizes, not the estimate's guess.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    CollectionFeedback, CollectionSnapshot, DataError, DataSource, RowMeasurement, Runner,
};

const APP: &str = r#"component App
  state count = 1000
  resource rows = rows(count) as shape list<number>
  action more(n: number)
    count = count + n
  view
    list virtualized=true scroll-start="end" height=600 width=400 overflow-x="hidden" estimated-item-height=100
      each x in rows key=x
        text `${x}` width="100%"
"#;

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
    Runner::boot(
        contract::compile(source).unwrap(),
        Data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn list(r: &Runner<Data>) -> CollectionSnapshot {
    r.collections().into_iter().next().unwrap()
}
/// A row's real size: tens are 160, the rest 40, against an estimate of 100.
fn size(i: usize) -> f64 {
    if i.is_multiple_of(10) {
        160.0
    } else {
        40.0
    }
}
/// A host with a 600-point port: it reports where it is (`at`), moves where
/// a correction says, clamped to its extent as a scroll view clamps, and
/// reports every mounted row at its real size, until the list asks for
/// nothing more. `apply` false: its first report comes before it applies
/// the opening correction. The offset it ends at.
fn host(r: &mut Runner<Data>, mut at: f64, mut apply: bool) -> f64 {
    let mut sequence = 0;
    for _ in 0..12 {
        let c = list(r);
        if let (Some(correction), true) = (c.correction, apply) {
            at = correction.offset.min((c.total_extent - 600.0).max(0.0));
        }
        apply = true;
        sequence = sequence.max(c.scroll_sequence);
        let changed = r
            .collection_feedback(CollectionFeedback {
                view: c.view,
                revision: c.revision,
                scroll_sequence: sequence,
                offset: at,
                port_main: 600.0,
                port_cross: 400.0,
                cross: 400.0,
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
            })
            .unwrap();
        if list(r).correction.is_none() && changed.receipts.is_empty() {
            break;
        }
    }
    at
}
fn at_end(r: &Runner<Data>, offset: f64) {
    let c = list(r);
    assert!(
        (c.total_extent - 600.0 - offset).abs() <= 0.5,
        "at the end: {offset} of {}",
        c.total_extent
    );
    let last = c
        .rows
        .iter()
        .find(|row| row.index + 1 == c.count)
        .expect("the last row is mounted");
    assert_eq!(last.size, size(last.index), "measured");
}

#[test]
fn the_list_opens_at_its_end_on_measured_sizes() {
    let mut r = boot(APP);
    let c = list(&r);
    // Before any report: the last rows, and the host told to start there.
    let first = c.rows.iter().map(|row| row.index).min().unwrap();
    assert!(
        first >= 990 && c.rows.iter().any(|row| row.index == 999),
        "{:?}",
        c.rows
    );
    let correction = c.correction.expect("the opening correction");
    assert_eq!(
        (correction.scroll_sequence, correction.offset),
        (0, c.total_extent)
    );
    let offset = host(&mut r, 0.0, true);
    at_end(&r, offset);
}

#[test]
fn a_host_that_reports_before_it_applies_the_correction_still_opens_at_the_end() {
    let mut r = boot(APP);
    let offset = host(&mut r, 0.0, false);
    at_end(&r, offset);
}

#[test]
fn once_open_the_list_follows_its_end_only_by_scroll_follow_end() {
    for follow in [false, true] {
        let source = if follow {
            APP.replace(
                "scroll-start=\"end\"",
                "scroll-start=\"end\" scrollFollowEnd=true",
            )
        } else {
            APP.to_string()
        };
        let mut r = boot(&source);
        let offset = host(&mut r, 0.0, true);
        r.act("more", vec![Value::Number(3.0)]).unwrap();
        let after = host(&mut r, offset, true);
        if follow {
            at_end(&r, after);
        } else {
            assert_eq!(after, offset, "the reader stays where it was");
        }
    }
}

#[test]
fn a_reader_who_scrolls_while_it_opens_keeps_their_place() {
    let mut r = boot(APP);
    let end = list(&r).total_extent - 600.0;
    host_once(&mut r, end, 0.0);
    // The reader drags up: travel in two reports running.
    host_once(&mut r, end - 1000.0, -2400.0);
    host_once(&mut r, end - 2000.0, -2400.0);
    let c = list(&r);
    assert!(c.correction.is_none(), "{:?}", c.correction);
    // Measuring around the reader anchors what they see (the rows below
    // measure shorter than their estimate), not the end.
    let offset = host(&mut r, end - 2000.0, true);
    let c = list(&r);
    assert!(
        c.total_extent - 600.0 - offset > 600.0,
        "{offset} of {}",
        c.total_extent
    );
}

#[test]
fn a_host_that_clamps_short_of_the_end_is_not_a_reader() {
    // UIKit clamps its port to rows laid out at their real size before the
    // runner has their measurements: a report short of the end, standing still.
    let mut r = boot(APP);
    let end = list(&r).total_extent - 600.0;
    host_once(&mut r, end, 0.0);
    host_once(&mut r, end - 640.0, 0.0);
    let offset = host(&mut r, end - 640.0, true);
    at_end(&r, offset);
}
#[test]
fn a_host_that_lays_out_rows_before_it_reports_them_opens_at_the_end() {
    // UIKit's port holds every mounted row at its real size, so between
    // reports it clamps to an extent the runner has not measured yet.
    let mut r = boot(APP);
    let mut at = 0.0;
    for _ in 0..12 {
        let c = list(&r);
        let laid_out: f64 = c.total_extent
            + c.rows
                .iter()
                .map(|row| size(row.index) - row.size)
                .sum::<f64>();
        if let Some(correction) = c.correction {
            at = correction.offset;
        }
        at = at.min((laid_out - 600.0).max(0.0));
        let measurements = c
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                size: size(row.index),
            })
            .collect();
        r.collection_feedback(CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence,
            offset: at,
            port_main: 600.0,
            port_cross: 400.0,
            cross: 400.0,
            measurements,
            focus_view: None,
            interaction_view: None,
        })
        .unwrap();
    }
    at_end(&r, at);
}
fn host_once(r: &mut Runner<Data>, offset: f64, velocity: f64) {
    let c = list(r);
    r.collection_feedback_filled(
        CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset,
            port_main: 600.0,
            port_cross: 400.0,
            cross: 400.0,
            measurements: vec![],
            focus_view: None,
            interaction_view: None,
        },
        exact_runner::CollectionFill {
            velocity,
            ..Default::default()
        },
    )
    .unwrap();
}

#[test]
fn rows_that_arrive_later_open_at_the_end_too() {
    // Before the first report, and after a report of the empty list.
    for report_first in [false, true] {
        let mut r = boot(&APP.replace("state count = 1000", "state count = 0"));
        if report_first {
            host(&mut r, 0.0, true);
        }
        r.act("more", vec![Value::Number(500.0)]).unwrap();
        let offset = host(&mut r, 0.0, true);
        at_end(&r, offset);
    }
}

#[test]
fn scroll_start_is_a_literal_on_a_virtualized_list() {
    contract::compile(&APP.replace("scroll-start=\"end\"", "scroll-start=\"start\"")).unwrap();
    for (from, to, id) in [
        (
            "scroll-start=\"end\"",
            "scroll-start=\"bottom\"",
            "lower-attr-value",
        ),
        ("scroll-start=\"end\"", "scroll-start=1", "lower-attr-value"),
        ("list virtualized=true", "list", "lower-list-virtualized"),
    ] {
        let e = contract::compile(&APP.replace(from, to))
            .unwrap_err()
            .to_string();
        assert!(e.contains(id) && e.contains("scroll-start"), "{to}: {e}");
    }
}
