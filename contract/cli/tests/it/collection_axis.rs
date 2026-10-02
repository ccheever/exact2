//! LLP 1070 H1–H4: a virtualized flex row scrolls horizontally, on the same
//! engine; the compiler refuses what would move its items' starts.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{CollectionFeedback, DataError, DataSource, ListAxis, RowMeasurement, Runner};

fn source(list: &str) -> String {
    format!("component App\n  resource rows = rows() as shape list<number>\n  view\n    column width=390\n      {list} testId=\"strip\"\n        each x in rows key=x\n          text `${{x}}` width=100 testId=`card-${{x}}`\n")
}
const STRIP: &str =
    "list virtualized=true display=\"flex\" height=132 overflow-x=\"scroll\" overflow-y=\"hidden\" estimated-item-width=100";

#[test]
fn a_flex_row_compiles_and_the_other_shapes_are_refused_by_name() {
    for list in [
        STRIP,
        "list virtualized=true display=\"flex\" flex-direction=\"row\" height=132 overflow-x=\"scroll\"",
        "list virtualized=true display=\"flex\" height=132 padding-top=8 padding-bottom=8 justify-content=\"flex-start\" flex-wrap=\"nowrap\"",
        "list virtualized=true display=\"block\" height=132 estimated-item-height=40",
    ] {
        contract::compile(&source(list)).unwrap_or_else(|e| panic!("{list}: {e}"));
    }
    for (list, id) in [
        (
            "list virtualized=true display=\"flex\" overflow-x=\"scroll\"",
            "lower-collection-cross",
        ),
        (
            "list virtualized=true display=\"flex\" height=\"auto\"",
            "lower-collection-cross",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 flex-direction=\"row-reverse\"",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 flex-direction=\"column\"",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true height=132 flex-direction=\"row\"",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"grid\" height=132",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 flex-wrap=\"wrap\"",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 justify-content=\"center\"",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 gap=8",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 direction=\"rtl\"",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 padding-left=8",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 overflow-y=\"scroll\"",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 overflow-x=\"hidden\"",
            "lower-collection-flow",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 estimated-item-height=40",
            "lower-collection-estimate",
        ),
        (
            "list virtualized=true height=132 estimated-item-width=40",
            "lower-collection-estimate",
        ),
        (
            "list display=\"flex\" height=132 estimated-item-width=40",
            "lower-list-virtualized",
        ),
        (
            "list virtualized=true display=\"flex\" height=132 estimated-item-width=0",
            "lower-list-height",
        ),
    ] {
        let error = contract::compile(&source(list)).unwrap_err().to_string();
        assert!(error.contains(id), "{list}: {error}: expected {id}");
    }
    let reorder = "component App\n  state n = \"\"\n  resource rows = rows() as shape list<number>\n  action drop(item: string, before: option<string>)\n    n = item\n  view\n    list id=\"s\" reorderdrop=drop virtualized=true display=\"flex\" height=132\n      each x in rows key=x\n        text `${x}`\n";
    let error = contract::compile(reorder).unwrap_err().to_string();
    assert!(error.contains("lower-collection-reorder"), "{error}");
}

struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            (0..1000).map(|i| Value::Number(i as f64)).collect(),
        ))
    }
}
fn boot(list: &str) -> Runner<Rows> {
    Runner::boot(
        contract::compile(&source(list)).unwrap(),
        Rows,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
/// A report at `offset` along the main axis, measuring each mounted row
/// at `size(index)`.
fn report(r: &mut Runner<Rows>, offset: f64, size: impl Fn(usize) -> f64) {
    let c = r.collections().pop().unwrap();
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
        scroll_sequence: c.scroll_sequence + 1,
        offset,
        port_main: 390.0,
        port_cross: 132.0,
        cross: 132.0,
        measurements,
        focus_view: None,
        interaction_view: None,
    })
    .unwrap();
}

fn report_bare(r: &mut Runner<Rows>, offset: f64) {
    let c = r.collections().pop().unwrap();
    r.collection_feedback(CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        offset,
        port_main: 390.0,
        port_cross: 132.0,
        cross: 132.0,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    })
    .unwrap();
}

#[test]
fn a_row_list_is_horizontal_its_wrappers_are_flex_items_and_its_spacers_widths() {
    let mut r = boot(STRIP);
    let c = r.collections().pop().unwrap();
    assert_eq!(c.axis, ListAxis::Horizontal);
    assert!(c.json().contains("\"axis\":\"x\""), "{}", c.json());
    // The estimate is the row axis's: 1,000 × 100.
    assert_eq!(c.total_extent, 100_000.0);
    report(&mut r, 50_000.0, |_| 100.0);
    let c = r.collections().pop().unwrap();
    assert!(
        c.rows.len() < 20,
        "a window of cards, not the strip: {}",
        c.rows.len()
    );
    let k = r.kernel();
    let list = k.node(c.view).unwrap();
    assert_eq!(
        list.style.min_width,
        exact_kernel::Dimension::Points(0.0),
        "the compiler's min-width: 0"
    );
    let wrapper = k.node(c.rows[0].view).unwrap();
    assert_eq!(wrapper.style.flex_shrink, 0.0);
    assert_eq!(wrapper.style.flex_grow, 0.0);
    assert_eq!(
        wrapper.style.width,
        exact_kernel::Dimension::Auto,
        "a card is its own width"
    );
    let spacer = k.node(list.children()[0]).unwrap();
    assert_eq!(
        spacer.style.width,
        exact_kernel::Dimension::Points(c.rows[0].start as f32)
    );
    // Laid out, the strip is the column's width and the cards sit in a row.
    let root = r.roots()[0];
    r.kernel_mut()
        .compute_layout(root, exact_kernel::Offer::definite(390.0, 844.0))
        .unwrap();
    let k = r.kernel();
    let list = k.node(c.view).unwrap();
    assert_eq!(
        list.frame.width, 390.0,
        "the spacers' extent does not widen the strip"
    );
    let (a, b) = (
        k.node(c.rows[0].view).unwrap(),
        k.node(c.rows[1].view).unwrap(),
    );
    assert_eq!(b.frame.x - a.frame.x, 100.0);
    assert_eq!(a.frame.y, b.frame.y);
}

#[test]
fn a_row_list_anchors_first_measurements_and_lets_a_remeasured_card_move_the_strip() {
    // @ref LLP 1070 Q3 (a).
    let left_of_view = |r: &Runner<Rows>, offset: f64| {
        let c = r.collections().pop().unwrap();
        c.rows
            .iter()
            .find(|row| row.start + row.size <= offset)
            .map(|row| row.index)
            .expect("overscan left of the port")
    };
    for (list, horizontal) in [
        (STRIP, true),
        (
            "list virtualized=true height=390 overflow-x=\"hidden\" estimated-item-height=100",
            false,
        ),
    ] {
        let mut r = boot(list);
        // Mount the window at 50,000 at its estimates, measuring nothing.
        report_bare(&mut r, 50_000.0);
        let far = left_of_view(&r, 50_000.0);
        // Its first measurement is 50 more than the estimate: every host
        // holds what shows.
        report(&mut r, 50_000.0, |i| if i == far { 150.0 } else { 100.0 });
        let c = r.collections().pop().unwrap();
        assert_eq!(c.correction.map(|c| c.offset), Some(50_050.0), "{list}");
        // Measured again 50 wider: a real change. A vertical list anchors
        // it; a row list lets the strip move, as Chrome does on x.
        report(&mut r, 50_050.0, |i| if i == far { 200.0 } else { 100.0 });
        let c = r.collections().pop().unwrap();
        let corrected = c.correction.map(|c| c.offset);
        if horizontal {
            assert_eq!(corrected, None, "{list}");
        } else {
            assert_eq!(corrected, Some(50_100.0), "{list}");
        }
    }
}
