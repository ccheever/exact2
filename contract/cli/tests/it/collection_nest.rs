//! LLP 1070 stage 4: a virtualized list in a virtualized list's row. It
//! lives and dies with the row (N1), keeps its position across the row's
//! retirement by default (Q1), pins its outer row (N5), and builds only what
//! it owes while the outer list moves (F2).
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    CollectionFeedback, CollectionFill, CollectionSnapshot, DataError, DataSource, RowMeasurement,
    Runner,
};

fn feed(inner: &str) -> String {
    format!("shape Post\n  id: number\n  cards: list<number>\ncomponent App\n  resource posts = posts() as shape list<Post>\n  view\n    list virtualized=true height=600 width=400 overflow-x=\"hidden\" estimated-item-height=200 testId=\"feed\"\n      each p in posts key=p.id\n        column width=\"100%\" height=200 testId=`post-${{p.id}}`\n          {inner}\n            each t in p.cards key=t\n              text `${{t}}` width=100 height=100 testId=`card-${{p.id}}-${{t}}`\n")
}
const STRIP: &str = "list virtualized=true display=\"flex\" height=120 overflow-x=\"scroll\" overflow-y=\"hidden\" estimated-item-width=100 testId=`strip-${p.id}`";

#[test]
fn one_level_of_constant_nesting_compiles_and_the_rest_is_refused_by_name() {
    for inner in [
        STRIP,
        "list virtualized=true height=150 overflow-x=\"hidden\" estimated-item-height=100",
        "list virtualized=true max-height=150 overflow-x=\"hidden\"",
        &format!("{STRIP} scroll-restoration=\"manual\""),
    ] {
        contract::compile(&feed(inner)).unwrap_or_else(|e| panic!("{inner}: {e}"));
    }
    for (inner, id) in [
        (
            "list virtualized=true flex=1 overflow-x=\"hidden\"",
            "lower-collection-unbounded",
        ),
        (
            &format!("{STRIP} scroll-restoration=\"keep\"") as &str,
            "lower-attr-value",
        ),
    ] {
        let error = contract::compile(&feed(inner)).unwrap_err().to_string();
        assert!(error.contains(id), "{inner}: {error}: expected {id}");
    }
    // Two levels down.
    let deep = "shape Post\n  id: number\ncomponent App\n  resource posts = posts() as shape list<Post>\n  view\n    list virtualized=true height=600\n      each p in posts key=p.id\n        list virtualized=true height=300\n          each q in posts key=q.id\n            list virtualized=true height=100\n              each r in posts key=r.id\n                text \"x\"\n";
    let error = contract::compile(deep).unwrap_err().to_string();
    assert!(error.contains("lower-collection-depth"), "{error}");
    let reorder = "shape Post\n  id: number\ncomponent App\n  state n = \"\"\n  resource posts = posts() as shape list<Post>\n  action drop(item: string, before: option<string>) writes n\n    n = item\n  view\n    list virtualized=true height=600\n      each p in posts key=p.id\n        list id=\"inner\" reorderdrop=drop virtualized=true height=300\n          each q in posts key=q.id\n            text \"x\"\n";
    let error = contract::compile(reorder).unwrap_err().to_string();
    assert!(error.contains("lower-collection-reorder"), "{error}");
}

struct Data {
    posts: usize,
}
impl DataSource for Data {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(match source {
            "posts" => {
                let cards = Value::list((0..200).map(|i| Value::Number(i as f64)).collect());
                Value::list(
                    (0..self.posts)
                        .map(|i| Value::record(vec![Value::Number(i as f64), cards.clone()]))
                        .collect(),
                )
            }
            _ => return Err(DataError::UnknownSource(source.into())),
        })
    }
}
fn boot(inner: &str, posts: usize) -> Runner<Data> {
    let plan = contract::compile(&feed(inner)).unwrap();
    Runner::boot(
        plan,
        Data { posts },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn view(r: &Runner<Data>, test_id: &str) -> Option<u32> {
    let k = r.kernel();
    Some(k.node_by_key(*k.find_by_test_id(test_id).first()?)?.id)
}
fn snapshot(r: &Runner<Data>, list: u32) -> Option<CollectionSnapshot> {
    r.collections().into_iter().find(|c| c.view == list)
}
/// A report for `list` at `offset`, measuring every mounted row at `size`,
/// with a port of `port` on its main axis.
fn report(
    r: &mut Runner<Data>,
    list: u32,
    offset: f64,
    port: f64,
    size: f64,
    fill: CollectionFill,
    focus: Option<u32>,
) {
    let c = snapshot(r, list).expect("mounted list");
    r.collection_feedback_filled(
        CollectionFeedback {
            view: list,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
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
                    size,
                })
                .collect(),
            focus_view: focus,
            interaction_view: None,
        },
        fill,
    )
    .unwrap();
}
fn feed_at(r: &mut Runner<Data>, offset: f64) {
    let feed = view(r, "feed").unwrap();
    report(
        r,
        feed,
        offset,
        600.0,
        200.0,
        CollectionFill::default(),
        None,
    );
    report(
        r,
        feed,
        offset,
        600.0,
        200.0,
        CollectionFill::default(),
        None,
    );
}

#[test]
fn an_inner_list_lives_with_its_row_and_names_its_parent() {
    let mut r = boot(STRIP, 100);
    feed_at(&mut r, 0.0);
    let feed = view(&r, "feed").unwrap();
    let strip = view(&r, "strip-0").expect("row 0's strip");
    let inner = snapshot(&r, strip).unwrap();
    assert_eq!(inner.parent, Some(feed));
    assert!(inner.json().contains(&format!("\"parent\":{feed}")));
    assert!(
        inner.rows.len() < 20,
        "the strip is windowed: {}",
        inner.rows.len()
    );
    // Far down the feed, row 0 and its strip are gone, in the same batch.
    feed_at(&mut r, 10_000.0);
    assert!(view(&r, "strip-0").is_none());
    assert!(view(&r, "card-0-0").is_none());
    let lists = r.collections();
    assert!(
        lists.len() <= 1 + 12,
        "the feed and its window's strips: {}",
        lists.len()
    );
    assert!(lists.iter().filter(|c| c.parent == Some(feed)).count() >= 3);
}

#[test]
fn an_inner_list_keeps_its_position_by_default_as_a_key_and_an_offset() {
    let mut r = boot(STRIP, 100);
    feed_at(&mut r, 0.0);
    let strip = view(&r, "strip-1").unwrap();
    // The reader scrolls row 1's strip 1,234 px: item 12, 34 px in.
    report(
        &mut r,
        strip,
        1234.0,
        400.0,
        100.0,
        CollectionFill::default(),
        None,
    );
    report(
        &mut r,
        strip,
        1234.0,
        400.0,
        100.0,
        CollectionFill::default(),
        None,
    );
    feed_at(&mut r, 10_000.0);
    assert!(view(&r, "strip-1").is_none());
    let state = exact_runner::agent::state(&r);
    assert!(state.contains("\"kept\":[{"), "{state}");
    assert!(state.contains("\"row\":\"n:1\""), "{state}");
    assert!(state.contains("\"key\":\"n:12\",\"within\":34"), "{state}");
    // Back: it is built at item 12 and told to move there before it paints.
    feed_at(&mut r, 0.0);
    let strip = view(&r, "strip-1").unwrap();
    let c = snapshot(&r, strip).unwrap();
    assert!(c.restored);
    assert_eq!(c.correction.map(|c| c.offset), Some(1234.0));
    assert!(
        c.rows.iter().any(|row| row.index == 12),
        "{:?}",
        c.rows.iter().map(|r| r.index).collect::<Vec<_>>()
    );
    assert!(view(&r, "card-1-12").is_some());
    // Its neighbours, never scrolled, start at their starts.
    let other = snapshot(&r, view(&r, "strip-2").unwrap()).unwrap();
    assert!(!other.restored && other.correction.is_none());
}

#[test]
fn manual_restoration_and_a_departed_row_keep_nothing() {
    let mut r = boot(&format!("{STRIP} scroll-restoration=\"manual\""), 100);
    feed_at(&mut r, 0.0);
    let strip = view(&r, "strip-1").unwrap();
    report(
        &mut r,
        strip,
        1234.0,
        400.0,
        100.0,
        CollectionFill::default(),
        None,
    );
    feed_at(&mut r, 10_000.0);
    assert!(exact_runner::agent::state(&r).contains("\"kept\":[]"));
    feed_at(&mut r, 0.0);
    let c = snapshot(&r, view(&r, "strip-1").unwrap()).unwrap();
    assert!(!c.restored && c.correction.is_none());
}

#[test]
fn an_inner_pin_pins_its_outer_row_and_the_chain_survives_the_feed_report() {
    let mut r = boot(STRIP, 100);
    feed_at(&mut r, 0.0);
    let strip = view(&r, "strip-1").unwrap();
    let card = view(&r, "card-1-2").unwrap();
    // The host reports the focus to its nearest owner, the strip.
    report(
        &mut r,
        strip,
        0.0,
        400.0,
        100.0,
        CollectionFill::default(),
        Some(card),
    );
    // The feed's own report says nothing of it; far away, row 1 stays.
    feed_at(&mut r, 10_000.0);
    assert!(
        view(&r, "strip-1").is_some(),
        "the focused card's row stays mounted"
    );
    assert_eq!(view(&r, "card-1-2"), Some(card));
    // Focus leaves: the chain goes, and so does the row.
    report(
        &mut r,
        strip,
        0.0,
        400.0,
        100.0,
        CollectionFill::default(),
        None,
    );
    feed_at(&mut r, 10_000.0);
    assert!(view(&r, "strip-1").is_none());
}

#[test]
fn while_the_feed_moves_an_inner_list_builds_only_what_it_owes() {
    let mut r = boot(STRIP, 100);
    feed_at(&mut r, 0.0);
    let strip = view(&r, "strip-0").unwrap();
    let moving = CollectionFill {
        ancestor_moving: true,
        ..Default::default()
    };
    // A first report, far along the strip: unlimited at rest, owed only now.
    report(&mut r, strip, 5000.0, 400.0, 100.0, moving, None);
    let owed = snapshot(&r, strip).unwrap();
    let shown: Vec<_> = owed
        .rows
        .iter()
        .map(|row| row.index)
        .filter(|i| (50..54).contains(i))
        .collect();
    assert_eq!(
        shown.len(),
        4,
        "what shows is built: {:?}",
        owed.rows.iter().map(|r| r.index).collect::<Vec<_>>()
    );
    assert!(
        owed.rows.iter().filter(|row| row.index >= 44).count() <= 6,
        "no overscan while the feed moves"
    );
    report(
        &mut r,
        strip,
        5000.0,
        400.0,
        100.0,
        CollectionFill::default(),
        None,
    );
    let rest = snapshot(&r, strip).unwrap();
    assert!(
        rest.rows.iter().filter(|row| row.index >= 44).count() > 6,
        "at rest the overscan fills"
    );
}

/// An inner list's first rows, before any host lays it out, are what its own
/// literal size shows, not the outer port's: a 100 pt inbox in a 600 pt feed
/// builds two rows and one more, not seven.
#[test]
fn an_inner_list_first_builds_what_its_own_size_shows() {
    let inbox = "list virtualized=true height=100 estimated-item-height=100 testId=`inbox-${p.id}`";
    let mut r = boot(inbox, 100);
    feed_at(&mut r, 0.0);
    let inbox = view(&r, "inbox-0").expect("row 0's inbox");
    let rows = snapshot(&r, inbox).unwrap().rows.len();
    assert_eq!(rows, 2, "ceil(100 / 100) + 1 rows");
}

/// Items above a kept position that were measured apart from their estimate
/// (80 against 100) come back as estimates in a new list: their difference is
/// spread over those estimates, so the kept item starts where it did and the
/// offset comes back exactly, not only the item (LLP 1070 §4.2).
#[test]
fn a_kept_position_comes_back_at_the_same_offset_over_measured_items() {
    let mut r = boot(STRIP, 100);
    feed_at(&mut r, 0.0);
    let strip = view(&r, "strip-1").unwrap();
    // The reader drags item by item to 1,234 px, every item measured at 80.
    let mut offset = 0.0;
    while offset < 1234.0 {
        offset = (offset + 300.0_f64).min(1234.0);
        for _ in 0..2 {
            report(
                &mut r,
                strip,
                offset,
                400.0,
                80.0,
                CollectionFill::default(),
                None,
            );
            if let Some(c) = snapshot(&r, strip).unwrap().correction {
                offset = c.offset;
            }
        }
    }
    let left = offset;
    feed_at(&mut r, 10_000.0);
    assert!(view(&r, "strip-1").is_none());
    feed_at(&mut r, 0.0);
    let strip = view(&r, "strip-1").unwrap();
    let c = snapshot(&r, strip).unwrap();
    let back = c.correction.map(|c| c.offset).expect("told where to start");
    assert!(
        (back - left).abs() <= 0.5,
        "the offset it left at: {back} against {left}"
    );
}
