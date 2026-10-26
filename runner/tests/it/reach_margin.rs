//! `reach-margin` sets how far ahead of an edge `reachstart` and `reachend`
//! fire (@ref LLP 1145 D4). 200 rows of 32 px in a 320 px port: the last row
//! starts at 6368.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{CollectionFeedback, DataError, DataSource, Runner};

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
  action onStart
    starts = starts + 1
  action onEnd
    ends = ends + 1
  action grow
    n = n + 20
  view
    list virtualized=true height=320 estimated-item-height=32 reachstart=onStart reachend=onEnd{margin}
      for x in range(n) key=x
        text `${{x}}` height=32
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

/// A report at `offset`, measuring every mounted row at 32 px.
fn send(r: &mut Runner<NoData>, offset: f64) {
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
    let result = r.collection_feedback(feedback).unwrap();
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
