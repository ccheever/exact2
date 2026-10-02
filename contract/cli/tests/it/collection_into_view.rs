//! LLP 1070.000: `scrollIntoView` brings a virtualized list's row into view
//! by key, aligned as the web aligns it, corrected until it lands.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    CollectionFeedback, CollectionSnapshot, DataError, DataSource, RowMeasurement, Runner,
};

const APP: &str = r#"shape Post
  id: number
  cards: list<number>
component App
  state shown = 0
  resource posts = posts() as shape list<Post>
  action go(n: number)
    shown = n
    scrollIntoView("feed", n, block="center")
  action goEnd(n: number)
    shown = n
    scrollIntoView("feed", n, block="end")
  action card(post: number, n: number)
    shown = n
    scrollIntoView("strip", n, row=post, inline="center", block="start")
  view
    list id="feed" virtualized=true height=600 width=400 overflow-x="hidden" estimated-item-height=100 testId="feed"
      each p in posts key=p.id
        column width="100%" testId=`post-${p.id}`
          text `${p.id}` height=40
          when p.id % 10 == 0
            list id="strip" virtualized=true display="flex" height=120 overflow-x="scroll" overflow-y="hidden" estimated-item-width=100
              each t in p.cards key=t
                text `${t}` width=60 height=100
"#;

#[test]
fn the_command_is_checked_by_name() {
    contract::compile(APP).unwrap();
    for (statement, id) in [
        (
            r#"scrollIntoView("feed", n, block="middle")"#,
            "type-scroll-into-view",
        ),
        (
            r#"scrollIntoView("feed", n, behavior="smooth")"#,
            "type-scroll-into-view",
        ),
        (
            r#"scrollIntoView("feed", n, align="start")"#,
            "type-scroll-into-view",
        ),
        (r#"scrollIntoView(n, n)"#, "type-scroll-into-view"),
        (r#"scrollIntoView("feed")"#, "type-scroll-into-view"),
    ] {
        let source = APP.replace(r#"scrollIntoView("feed", n, block="end")"#, statement);
        let error = contract::compile(&source).unwrap_err().to_string();
        assert!(error.contains(id), "{statement}: {error}");
    }
}

struct Data;
impl DataSource for Data {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        let cards = Value::list((0..300).map(|i| Value::Number(i as f64)).collect());
        Ok(Value::list(
            (0..1000)
                .map(|i| Value::record(vec![Value::Number(i as f64), cards.clone()]))
                .collect(),
        ))
    }
}
fn boot() -> Runner<Data> {
    Runner::boot(
        contract::compile(APP).unwrap(),
        Data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn list(r: &Runner<Data>, horizontal: bool) -> Option<CollectionSnapshot> {
    r.collections().into_iter().find(|c| {
        (c.axis == exact_runner::ListAxis::Horizontal) == horizontal
            && (horizontal || c.parent.is_none())
    })
}
/// A host: it moves the port where a correction says, then reports every
/// mounted row at its real size (feed rows 40, or 160 with a strip; cards
/// 60), until the list asks for nothing more. The offset it ends at.
fn host(
    r: &mut Runner<Data>,
    view: u32,
    mut offset: f64,
    port: f64,
    size: impl Fn(usize) -> f64,
) -> f64 {
    let mut sequence = 0;
    for _ in 0..8 {
        let c = r
            .collections()
            .into_iter()
            .find(|c| c.view == view)
            .unwrap();
        if let Some(correction) = c.correction {
            offset = correction.offset;
        }
        sequence = sequence.max(c.scroll_sequence);
        let changed = r
            .collection_feedback(CollectionFeedback {
                view,
                revision: c.revision,
                scroll_sequence: sequence,
                offset,
                port_main: port,
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
        let after = r
            .collections()
            .into_iter()
            .find(|c| c.view == view)
            .unwrap();
        if after.correction.is_none() && changed.receipts.is_empty() {
            break;
        }
    }
    offset
}
fn feed_size(i: usize) -> f64 {
    if i.is_multiple_of(10) {
        160.0
    } else {
        40.0
    }
}
fn state(r: &Runner<Data>) -> String {
    exact_runner::agent::state(r)
}

#[test]
fn a_row_far_away_lands_centred_on_its_measured_size() {
    let mut r = boot();
    let feed = list(&r, false).unwrap().view;
    host(&mut r, feed, 0.0, 600.0, feed_size);
    r.act("go", vec![Value::Number(503.0)]).unwrap();
    let c = list(&r, false).unwrap();
    assert!(
        c.rows.iter().any(|row| row.index == 503),
        "the destination is built in the same commit"
    );
    assert!(
        c.rows.len() < 40,
        "the destination's window, not the path: {}",
        c.rows.len()
    );
    assert!(
        state(&r).contains("\"status\":\"pending\""),
        "{}",
        state(&r)
    );
    let offset = host(&mut r, feed, 0.0, 600.0, feed_size);
    let c = list(&r, false).unwrap();
    let row = c.rows.iter().find(|row| row.index == 503).unwrap();
    assert_eq!(row.size, 40.0);
    assert!(
        (row.start + row.size / 2.0 - 300.0 - offset).abs() <= 0.5,
        "centred: start {} offset {offset}",
        row.start
    );
    assert!(state(&r).contains("\"status\":\"done\""), "{}", state(&r));
}

#[test]
fn the_last_row_clamps_to_the_end_and_a_missing_key_is_refused() {
    let mut r = boot();
    let feed = list(&r, false).unwrap().view;
    host(&mut r, feed, 0.0, 600.0, feed_size);
    r.act("goEnd", vec![Value::Number(999.0)]).unwrap();
    let offset = host(&mut r, feed, 0.0, 600.0, feed_size);
    let c = list(&r, false).unwrap();
    assert!(
        (c.total_extent - 600.0 - offset).abs() <= 0.5,
        "at the end: {offset} of {}",
        c.total_extent
    );
    r.act("go", vec![Value::Number(5000.0)]).unwrap();
    let s = state(&r);
    assert!(s.contains("refused: no row keyed n:5000"), "{s}");
}

#[test]
fn a_reader_who_scrolls_cancels_the_request() {
    let mut r = boot();
    let feed = list(&r, false).unwrap().view;
    host(&mut r, feed, 0.0, 600.0, feed_size);
    r.act("go", vec![Value::Number(400.0)]).unwrap();
    // The reader drags somewhere else before the correction lands: two
    // reports of travel (one could be the browser clamping the port).
    for offset in [1234.0, 1300.0] {
        let c = list(&r, false).unwrap();
        r.collection_feedback_filled(
            CollectionFeedback {
                view: feed,
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
                velocity: -2400.0,
                ..Default::default()
            },
        )
        .unwrap();
    }
    assert!(
        state(&r).contains("\"status\":\"cancelled\""),
        "{}",
        state(&r)
    );
}

#[test]
fn a_card_in_a_strip_in_a_row_far_away_brings_both_into_view() {
    let mut r = boot();
    let feed = list(&r, false).unwrap().view;
    host(&mut r, feed, 0.0, 600.0, feed_size);
    r.act("card", vec![Value::Number(500.0), Value::Number(150.0)])
        .unwrap();
    // The outer row is at the feed's start; the strip holds card 150.
    let offset = host(&mut r, feed, 0.0, 600.0, feed_size);
    let c = list(&r, false).unwrap();
    let row = c
        .rows
        .iter()
        .find(|row| row.index == 500)
        .expect("the row is mounted");
    assert!(
        (row.start - offset).abs() <= 0.5,
        "the row starts the port: {} vs {offset}",
        row.start
    );
    let strip = r
        .collections()
        .into_iter()
        .find(|c| c.parent == Some(feed) && c.rows.iter().any(|x| x.index == 150))
        .expect("its strip, at card 150");
    let offset = host(&mut r, strip.view, 0.0, 400.0, |_| 60.0);
    let strip = r
        .collections()
        .into_iter()
        .find(|c| c.view == strip.view)
        .unwrap();
    let card = strip.rows.iter().find(|x| x.index == 150).unwrap();
    assert!(
        (card.start + card.size / 2.0 - 200.0 - offset).abs() <= 0.5,
        "centred inline: {} vs {offset}",
        card.start
    );
}
