//! LLP 1070.000: `scrollIntoView` brings a virtualized list's row into view
//! by key, aligned as the web aligns it, corrected until it lands.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    CollectionFeedback, CollectionSnapshot, DataError, DataSource, RowMeasurement, Runner,
};

const APP: &str = r#"shape Post
  id: number
  cards: number[]
component App
  state shown = 0
  resource posts = posts() as shape Post[]
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
      for p in posts key=p.id
        column width="100%" testId=`post-${p.id}`
          text `${p.id}` height=40
          if p.id % 10 == 0
            list id="strip" virtualized=true display="flex" height=120 overflow-x="scroll" overflow-y="hidden" estimated-item-width=100
              for t in p.cards key=t
                text `${t}` width=60 height=100
"#;

#[test]
fn the_command_is_checked_by_name() {
    contract::compile(APP).unwrap();
    // `smooth` is built (LLP 1070.000 §6.2).
    contract::compile(&APP.replace(
        r#"scrollIntoView("feed", n, block="end")"#,
        r#"scrollIntoView("feed", n, block="end", behavior="smooth")"#,
    ))
    .unwrap();
    for (statement, id) in [
        (
            r#"scrollIntoView("feed", n, block="middle")"#,
            "type-scroll-into-view",
        ),
        (
            r#"scrollIntoView("feed", n, behavior="slow")"#,
            "type-scroll-into-view",
        ),
        (
            r#"scrollIntoView("feed", n, align="start")"#,
            "type-scroll-into-view",
        ),
        (r#"scrollIntoView(n, n)"#, "type-scroll-into-view"),
        (r#"scrollIntoView(n)"#, "type-scroll-into-view"),
        (r#"scrollIntoView("cell", row=n)"#, "type-scroll-into-view"),
        (
            r#"scrollIntoView("cell", inline="left")"#,
            "type-scroll-into-view",
        ),
        (r#"scrollIntoView()"#, "type-scroll-into-view"),
        (
            r#"scrollIntoView("cell", offset=61)"#,
            "type-scroll-into-view",
        ),
        (
            r#"scrollIntoView("feed", n, offset="61")"#,
            "type-scroll-into-view",
        ),
    ] {
        let source = APP.replace(r#"scrollIntoView("feed", n, block="end")"#, statement);
        let error = contract::compile(&source).unwrap_err().to_string();
        assert!(error.contains(id), "{statement}: {error}");
    }
}

/// One positional argument is `Element.scrollIntoView()` on any element by
/// its `id`, as `focus("id")` names one (minesweeper F3): the runner hands
/// it to the host with its options, `none` where the web's default holds.
#[test]
fn an_elements_scroll_into_view_is_the_hosts() {
    let source = r#"component App
  state n = 7
  action go
    scrollIntoView(`cell-${n}`, block="nearest", inline="nearest")
  view
    column
      button "go" press=go testId="go"
      scroll height=100
        box id="cell-7" height=50
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let go = r.kernel().find_by_test_id("go")[0];
    let id = r.kernel().node_by_key(go).unwrap().id;
    r.dispatch(id, exact_runner::Event::Press).unwrap();
    let commands = r.take_commands();
    let [command] = commands.as_slice() else {
        panic!("{commands:?}")
    };
    assert_eq!(command.name, "scrollIntoView");
    assert_eq!(
        command.args,
        vec![
            Value::str("cell-7"),
            Value::str("nearest"),
            Value::str("nearest"),
            Value::NONE
        ]
    );
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
    offset: f64,
    port: f64,
    size: impl Fn(usize) -> f64,
) -> f64 {
    host_padded(r, view, offset, port, [0.0; 2], size)
}
/// `host`, laying out `padding` before the rows and after them.
fn host_padded(
    r: &mut Runner<Data>,
    view: u32,
    mut offset: f64,
    port: f64,
    padding: [f64; 2],
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
                padding,
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

/// A list UIKit draws under its bars (LLP 1130 D3) has their bands as
/// padding in a port that includes them; its host says what they cover. A
/// row aligned at the end lands above the bottom bar and one centred, in the
/// middle of what shows, as `scrollToItem(at:at:animated:)` aligns within the
/// adjusted content insets (the r26 chat's newest message under its composer).
#[test]
fn a_row_aligns_clear_of_what_the_platform_covers() {
    let mut r = boot();
    let feed = list(&r, false).unwrap().view;
    let (port, bands) = (600.0, [100.0, 80.0]);
    host_padded(&mut r, feed, -100.0, port, bands, feed_size);
    assert!(r.set_list_covered(feed, bands));
    let landed = |r: &mut Runner<Data>, n: usize| {
        let offset = host_padded(r, feed, -100.0, port, bands, feed_size);
        let row = list(r, false)
            .unwrap()
            .rows
            .into_iter()
            .find(|row| row.index == n)
            .unwrap();
        (row.start - offset, row.start + row.size - offset)
    };
    r.act("goEnd", vec![Value::Number(503.0)]).unwrap();
    let (_, end) = landed(&mut r, 503);
    assert!(
        (end - (port - bands[1])).abs() <= 0.5,
        "its end at the bottom bar's top: {end}"
    );
    r.act("go", vec![Value::Number(207.0)]).unwrap();
    let (start, end) = landed(&mut r, 207);
    let middle = (bands[0] + port - bands[1]) / 2.0;
    assert!(
        ((start + end) / 2.0 - middle).abs() <= 0.5,
        "centred in what shows: {start}…{end}"
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
                padding: [0.0; 2],
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

#[test]
fn a_scroll_event_restores_the_first_visible_key_and_offset_after_remount() {
    let source = r#"shape Post
  id: number
  cards: number[]
component App
  resource posts = posts() as shape Post[]
  state shown = true
  state saved = none
  state within = 0
  action moved(x: number, y: number, e: ScrollEvent)
    saved = e.firstVisibleKey
    within = e.firstVisibleOffset
  action hide
    shown = false
  action restore
    shown = true
    match saved
      case some(key)
        scrollIntoView("feed", key, offset=within)
      case none
        within = 0
  view
    column
      if shown
        list id="feed" virtualized=true height=600 width=400 estimated-item-height=100 scroll=moved
          for p in posts key=p.id
            text `${p.id}` height=(40 + p.id % 80)
"#;
    for key in ["p.id", "`${p.id}`"] {
        let source = source.replace("key=p.id", &format!("key={key}"));
        let mut r = Runner::boot(
            contract::compile(&source).unwrap(),
            Data,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let feed = list(&r, false).unwrap().view;
        host(&mut r, feed, 0.0, 600.0, |_| 100.0);
        host(&mut r, feed, 1861.0, 600.0, |_| 100.0);
        r.dispatch(
            feed,
            exact_runner::Event::scroll_payload("0,1861,400,100000,400,600").unwrap(),
        )
        .unwrap();
        assert_eq!(r.slot("saved"), Some(&Value::some(Value::str("18"))));
        assert_eq!(r.slot("within"), Some(&Value::Number(61.0)));
        r.act("hide", vec![]).unwrap();
        assert!(r.collections().is_empty());
        r.act("restore", vec![]).unwrap();
        let feed = list(&r, false).unwrap().view;
        let offset = host(&mut r, feed, 0.0, 600.0, |i| 80.0 + (i % 3) as f64 * 20.0);
        let c = list(&r, false).unwrap();
        let row = c.rows.iter().find(|row| row.index == 18).unwrap();
        assert!(
            (offset - row.start - 61.0).abs() <= 0.5,
            "{offset}, {row:?}"
        );
        assert!(state(&r).contains("\"status\":\"done\""), "{}", state(&r));
    }
}

/// Rows of 100.25: row 18 starts at 1804.5. Edges are drawn on device
/// pixels, each rounded to the nearest with a half up, so a port starting
/// less than half a device pixel above a row's start, or up to half one
/// below it, shows that row from its start (the event names it, at 0) and
/// none of the row before it; a port half a device pixel above shows a
/// pixel of the row before it.
#[test]
fn a_scroll_event_names_the_first_row_drawn_in_the_port() {
    let source = r#"component App
  resource posts = posts() as shape Post[]
  state saved = none
  state within = 0
  action moved(x: number, y: number, e: ScrollEvent)
    saved = e.firstVisibleKey
    within = e.firstVisibleOffset
  view
    list id="feed" virtualized=true height=600 width=400 estimated-item-height=100.25 scroll=moved
      for p in posts key=p.id
        text `${p.id}` height=100.25
shape Post
  id: number
  cards: number[]
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let feed = list(&r, false).unwrap().view;
    host(&mut r, feed, 1804.0, 600.0, |_| 100.25);
    let heard = |r: &mut Runner<Data>, top: &str| {
        let payload = format!("0,{top},400,100300,400,600");
        r.dispatch(feed, exact_runner::Event::scroll_payload(&payload).unwrap())
            .unwrap();
        let Some(Value::Option(Some(key))) = r.slot("saved").cloned() else {
            panic!("no key at {top}")
        };
        let Some(Value::Number(within)) = r.slot("within").cloned() else {
            panic!()
        };
        (
            key.as_str().unwrap().to_string(),
            (within * 1000.0).round() / 1000.0,
        )
    };
    // One device pixel a point: half of one is 0.5.
    assert_eq!(heard(&mut r, "1804"), ("17".into(), 99.75));
    assert_eq!(heard(&mut r, "1804.1"), ("18".into(), 0.0));
    assert_eq!(heard(&mut r, "1805"), ("18".into(), 0.0));
    assert_eq!(heard(&mut r, "1805.1"), ("18".into(), 0.6));
    // Two a point, at once: half of one is a quarter.
    r.set_display_scale(2.0).unwrap();
    assert_eq!(heard(&mut r, "1804.25"), ("17".into(), 100.0));
    assert_eq!(heard(&mut r, "1804.5"), ("18".into(), 0.0));
    assert_eq!(heard(&mut r, "1804.75"), ("18".into(), 0.0));
    assert_eq!(heard(&mut r, "1805"), ("18".into(), 0.5));
}

/// Edges are taken to a layout unit (1/64) before they round: at three
/// device pixels a point, row 0 ends at 100.834, drawn at 100.828125's
/// pixel, 100⅔, where a jump puts the port. The next row drawn is row 2:
/// row 1 has no height. Then rows 0 to 2 of 100.7, 0 and 0.05 all end at
/// that pixel, and the next row drawn is row 3.
#[test]
fn a_scroll_event_rounds_edges_as_they_are_drawn() {
    let source = r#"component App
  resource posts = posts() as shape Post[]
  state saved = none
  state within = 0
  action moved(x: number, y: number, e: ScrollEvent)
    saved = e.firstVisibleKey
    within = e.firstVisibleOffset
  view
    list id="feed" virtualized=true height=600 width=400 estimated-item-height=100 scroll=moved
      for p in posts key=p.id
        text `${p.id}`
shape Post
  id: number
  cards: number[]
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_display_scale(3.0).unwrap();
    let feed = list(&r, false).unwrap().view;
    for (sizes, first) in [([100.834, 0.0, 100.0], "2"), ([100.7, 0.0, 0.05], "3")] {
        host(&mut r, feed, 100.0, 600.0, |i| {
            sizes.get(i).copied().unwrap_or(100.0)
        });
        let payload = "0,100.66666666666667,400,100000,400,600";
        r.dispatch(feed, exact_runner::Event::scroll_payload(payload).unwrap())
            .unwrap();
        assert_eq!(r.slot("saved"), Some(&Value::some(Value::str(first))));
        assert_eq!(r.slot("within"), Some(&Value::Number(0.0)));
    }
}
