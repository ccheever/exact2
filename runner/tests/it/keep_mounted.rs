//! `keep-mounted` keeps the rows it names mounted wherever a virtualized
//! list's window is (@ref LLP 1010 §6.11): a transcript's first and last
//! message, so VoiceOver can move between them, and a playing row. 200 rows
//! of 32 px in a 320 px port.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{CollectionFeedback, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("queried {name}")
    }
}

fn boot(keep: &str) -> Runner<NoData> {
    let source = format!(
        "component App
  state n = 200
  state playing = 100
  action grow
    n = n + 20
  action play(at: number)
    playing = at
  view
    list virtualized=true height=320 estimated-item-height=32 keep-mounted={keep}
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

fn mounted(r: &Runner<NoData>) -> Vec<usize> {
    r.collections()[0]
        .rows
        .iter()
        .map(|row| row.index)
        .collect()
}

#[test]
fn the_kept_rows_stay_mounted_wherever_the_window_is() {
    let mut r = boot("[0, n - 1, playing]");
    send(&mut r, 0.);
    send(&mut r, 0.);
    let rows = mounted(&r);
    assert!(rows.contains(&199) && rows.contains(&100), "{rows:?}");
    // In the middle, three ports from either end: the first row is kept, and
    // nothing else outside the window is.
    send(&mut r, 3200.);
    let rows = mounted(&r);
    assert!(rows.contains(&0) && rows.contains(&199), "{rows:?}");
    assert!(
        rows.iter()
            .all(|&p| p == 0 || p == 199 || (90..=120).contains(&p)),
        "{rows:?}"
    );
    // At the end: the first row and the playing one are still there.
    send(&mut r, 6080.);
    let rows = mounted(&r);
    assert!(rows.contains(&0) && rows.contains(&100), "{rows:?}");
    // The other rows went as the window passed them.
    assert!(!rows.contains(&50), "{rows:?}");
}

#[test]
fn a_changed_set_mounts_its_new_rows_and_lets_the_old_ones_go() {
    let mut r = boot("[0, n - 1, playing]");
    send(&mut r, 0.);
    send(&mut r, 0.);
    // A row the window already holds: nothing new to measure.
    let before = r.collections()[0].revision;
    r.act("play", vec![Value::Number(5.)]).unwrap();
    assert!(!mounted(&r).contains(&100), "{:?}", mounted(&r));
    assert_eq!(r.collections()[0].revision, before);
    r.act("play", vec![Value::Number(150.)]).unwrap();
    let rows = mounted(&r);
    assert!(rows.contains(&150) && !rows.contains(&100), "{rows:?}");
    assert!(
        r.collections()[0].revision > before,
        "a host measures the new row"
    );
    // New rows at the end: the last row kept is the new last one.
    r.act("grow", vec![]).unwrap();
    let rows = mounted(&r);
    assert!(rows.contains(&219) && !rows.contains(&199), "{rows:?}");
}

#[test]
fn none_names_no_row_and_a_refused_value_keeps_none() {
    let mut r = boot("[first(range(n)), none, some(7)]");
    send(&mut r, 3200.);
    let rows = mounted(&r);
    assert!(rows.contains(&0) && rows.contains(&7), "{rows:?}");
    // More than 64 keys: the value is refused, the row unset, and it says so.
    let mut r = boot("range(65)");
    send(&mut r, 3200.);
    assert!(!mounted(&r).contains(&0), "{:?}", mounted(&r));
    assert!(
        r.journal().any(|line| line.contains("invalid keepMounted")),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
}

/// A kept row before a jump's target is mounted, and the rows between it and
/// the window are not: the jump settles on the run of rows that reaches the
/// target, as it does with nothing kept.
#[test]
fn a_jump_past_a_kept_row_settles() {
    for keep in ["[]", "[0]"] {
        let mut r = boot(keep);
        send(&mut r, 0.);
        send(&mut r, 0.);
        let view = r.collections()[0].view;
        r.scroll_into_view_at(view, "150", "start", "nearest")
            .unwrap();
        for _ in 0..20 {
            let c = &r.collections()[0];
            let offset = c.correction.as_ref().map_or(4800., |c| c.offset);
            send(&mut r, offset);
        }
        let status = r.into_view_json();
        assert!(status.contains("\"done\""), "{keep}: {status}");
        assert!(!r.collections()[0].seeking, "{keep}");
    }
}
