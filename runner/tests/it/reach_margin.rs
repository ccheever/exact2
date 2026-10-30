//! `reach-margin` sets how far ahead of an edge `reachstart` and `reachend`
//! fire (@ref LLP 1145 D4). 200 rows of 32 px in a 320 px port: the last row
//! starts at 6368.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{CollectionFeedback, CollectionFill, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("queried {name}")
    }
}

fn boot(margin: &str) -> Runner<NoData> {
    let source = format!(
        "component App
  state n = 200
  state starts = 0
  state ends = 0
  state older = 0
  action onStart
    starts = starts + 1
  action onEnd
    ends = ends + 1
  action grow
    n = n + 20
  action prepend
    n = n + 20
    older = older + 20
  view
    list virtualized=true height=320 estimated-item-height=32 reachstart=onStart reachend=onEnd{margin}
      for x in range(n) key=(x - older)
        text `${{x - older}}` height=32
"
    );
    Runner::boot(
        contract::compile(&source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// A report at `offset` at rest, measuring every mounted row at 32 px.
fn send(r: &mut Runner<NoData>, offset: f64) {
    fling(r, offset, 0.);
}

/// [`send`] with the scrollport moving at `velocity` px/s.
fn fling(r: &mut Runner<NoData>, offset: f64, velocity: f64) {
    let c = &r.collections()[0];
    let feedback = CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        offset,
        port_cross: 640.,
        port_main: 320.,
        cross: 640.,
        padding: [0.0; 2],
        measurements: c
            .rows
            .iter()
            .map(|row| exact_runner::RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                size: 32.,
            })
            .collect(),
        focus_view: None,
        interaction_view: None,
    };
    let fill = CollectionFill {
        velocity,
        ..Default::default()
    };
    let result = r.collection_feedback_filled(feedback, fill).unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
}

fn hits(r: &Runner<NoData>) -> [f64; 2] {
    ["starts", "ends"].map(|name| r.slot(name).unwrap().as_number().unwrap())
}

/// The first offset of `offsets`, scrolling down from 1000, at which `reachend` fires.
fn end_fires_at(margin: &str, offsets: &[f64]) -> Option<f64> {
    let mut r = boot(margin);
    send(&mut r, 1000.);
    offsets.iter().copied().find(|&offset| {
        send(&mut r, offset);
        hits(&r)[1] == 1.
    })
}

#[test]
fn a_larger_margin_fires_reachend_earlier_and_the_default_is_one_port() {
    let offsets: Vec<f64> = (0..=20).map(|i| 5000. + 64. * f64::from(i)).collect();
    // The last row's start, 6368, within the margin past the port's end.
    let default = end_fires_at("", &offsets);
    assert_eq!(default, Some(5768.), "one port: 5768 + 320 + 320 > 6368");
    assert_eq!(end_fires_at(" reach-margin=\"100%\"", &offsets), default);
    assert_eq!(end_fires_at(" reach-margin=\"320px\"", &offsets), default);
    assert_eq!(
        end_fires_at(" reach-margin=\"300%\"", &offsets),
        Some(5128.)
    );
    assert_eq!(
        end_fires_at(" reach-margin=\"0px 960px\"", &offsets),
        Some(5128.)
    );
    assert_eq!(end_fires_at(" reach-margin=\"0px\"", &offsets), Some(6088.));
}

#[test]
fn the_before_margin_sets_reachstart() {
    for (margin, fires) in [("", true), (" reach-margin=\"0px 100%\"", false)] {
        let mut r = boot(margin);
        send(&mut r, 200.);
        assert_eq!(hits(&r)[0] == 1., fires, "{margin}");
    }
}

#[test]
fn an_edge_past_the_mounted_window_rearms() {
    let mut r = boot(" reach-margin=\"400%\"");
    send(&mut r, 0.);
    assert_eq!(hits(&r), [1., 0.]);
    send(&mut r, 3000.);
    send(&mut r, 3000.);
    send(&mut r, 0.);
    assert_eq!(
        hits(&r),
        [2., 0.],
        "re-armed though rows 1280 px ahead were never mounted"
    );
    send(&mut r, 4800.);
    send(&mut r, 4800.);
    assert_eq!(hits(&r), [2., 1.], "4800 + 320 + 1280 reaches 6368");
    r.act("grow", vec![]).unwrap();
    send(&mut r, 4800.);
    assert_eq!(
        hits(&r),
        [2., 1.],
        "the grown end is 1280 px past the margin"
    );
    send(&mut r, 5500.);
    assert_eq!(hits(&r), [2., 2.]);
}

/// Whether `reachstart` fires at `offset`, moving at `velocity`, after a report
/// at rest at 1000.
fn start_fires(margin: &str, offset: f64, velocity: f64) -> bool {
    let mut r = boot(margin);
    send(&mut r, 1000.);
    fling(&mut r, offset, velocity);
    hits(&r)[0] == 1.
}

#[test]
fn a_fling_toward_an_edge_fires_it_as_far_ahead_as_the_window_leads() {
    // At 4,000 px/s the window leads a quarter second of travel, capped at
    // two ports: 320 + 640 before the port at 960 reaches row 0, which ends
    // at 32.
    assert!(start_fires("", 960., -4000.), "the window's lead");
    assert!(!start_fires("", 960., 0.), "one port at rest");
    assert!(
        !start_fires("", 960., 4000.),
        "travel away leads the other end"
    );
    assert!(start_fires("", 991., -4000.));
    assert!(!start_fires("", 992., -4000.));
    // 1,000 px/s leads 250 px more than the margin.
    assert!(start_fires("", 601., -1000.));
    assert!(!start_fires("", 602., -1000.));
    // An explicit margin is the band at rest; travel adds the same lead.
    assert!(start_fires(" reach-margin=\"0px\"", 640., -4000.));
    assert!(!start_fires(" reach-margin=\"0px\"", 640., 0.));
    assert!(start_fires(" reach-margin=\"200%\"", 671., 0.));
    assert!(!start_fires(" reach-margin=\"200%\"", 672., 0.));
    assert!(start_fires(" reach-margin=\"200%\"", 1311., -4000.));
    assert!(!start_fires(" reach-margin=\"200%\"", 1312., -4000.));
    assert!(start_fires(" reach-margin=\"100px 0px\"", 381., -1000.));
    assert!(!start_fires(" reach-margin=\"100px 0px\"", 382., -1000.));
}

#[test]
fn an_edge_a_fling_fired_rearms_once_outside_the_band_at_rest() {
    let mut r = boot("");
    send(&mut r, 1000.);
    fling(&mut r, 960., -4000.);
    assert_eq!(hits(&r), [1., 0.]);
    fling(&mut r, 900., -4000.);
    assert_eq!(hits(&r), [1., 0.], "still within the band: disarmed");
    // Stopped at 900, row 0 is outside the one-port band: it re-arms, and
    // the next fling toward it fires again.
    send(&mut r, 900.);
    assert_eq!(hits(&r), [1., 0.]);
    fling(&mut r, 900., -4000.);
    assert_eq!(hits(&r), [2., 0.]);
}

/// Excord's fling: `reachstart` fires 960 px out, a message arrives at the
/// end, the reader travels on to 100, and a page of 20 rows (640 px) lands,
/// which moves the port to 740.
fn paged(velocity: f64) -> Runner<NoData> {
    let mut r = boot("");
    send(&mut r, 1000.);
    fling(&mut r, 960., -4000.);
    fling(&mut r, 900., -4000.);
    r.act("grow", vec![]).unwrap();
    fling(&mut r, 900., -4000.);
    fling(&mut r, 900., -4000.);
    assert_eq!(hits(&r), [1., 0.], "rows past the other end re-arm nothing");
    fling(&mut r, 100., -4000.);
    assert_eq!(hits(&r), [1., 0.]);
    r.act("prepend", vec![]).unwrap();
    let correction = r.collections()[0].correction.unwrap();
    assert_eq!(correction.offset, 740.);
    fling(&mut r, 740., velocity);
    r
}

#[test]
fn a_page_that_lands_within_a_flings_band_rearms_its_edge_by_the_band_at_rest() {
    // Still flinging, the new first row is within the band: the edge re-armed
    // when the page landed and fires for the next page.
    assert_eq!(hits(&paged(-4000.)), [2., 0.]);
    // Stopped there, it re-arms and waits for the reader, as a page smaller
    // than the band at rest would have left it.
    let mut r = paged(0.);
    assert_eq!(hits(&r), [1., 0.]);
    send(&mut r, 400.);
    assert_eq!(hits(&r), [1., 0.]);
    send(&mut r, 300.);
    assert_eq!(hits(&r), [2., 0.]);
}
