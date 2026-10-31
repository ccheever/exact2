//! LLP 1084 D7 on the Mac's grouped form (LLP 1115.002 D6): a row keeps
//! its 36 points under `border-box`, rows stand a point apart, and the
//! separator is drawn in that gap, 10 points in from both ends of the card.
//! One test, its own process: the column is the process's.
use exact_kernel::style::metrics::{set_column, Column};
use exact_kernel::{Color, ColorValue, Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SOURCE: &str = r#"style Native
  appearance="auto"
component App
  action go
  view
    column testId="root" width=402 height=874
      list appearance="auto" flex=1
        section
          row testId="first"
            view width=4 height=4
          row width="100%" testId="full"
            image "symbol:sf/star"
            text "Full width" flex=1
            text "42" testId="value"
          button press=go testId="last"
            text "About"
          button press=go destructive=true testId="delete"
            text "Delete"
          row delete=go testId="swiped"
            text "Swiped"
          button appearance="auto" press=go width="100%" testId="native-full"
            text "Wide"
          button class=Native press=go destructive=true width="100%" testId="native-pushed"
            text "Erase"
          scroll swipeContent="written-row" swipeTrailing="written-act" testId="written" width="100%" overflow-x="scroll" overflow-y="hidden" display="flex"
            row id="written-row" width="100%" flex-shrink=0 testId="written-row"
              text "Written"
            button id="written-act" press=go flex-shrink=0
              text "Flag"
          row testId="end"
            text "End"
"#;

#[test]
fn the_macs_rows_keep_their_height_and_their_separator_stands_in_the_gap() {
    set_column(Column::Mac);
    let plan = contract::bake(contract::compile(SOURCE).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    let k = r.kernel();
    let id = |t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id;
    let frame = |t: &str| k.node(id(t)).unwrap().frame;
    let (first, full) = (frame("first"), frame("full"));
    assert_eq!(
        first.height, 36.0,
        "a row is 36 points at least, its padding in it"
    );
    assert_eq!(full.y - first.y, 37.0, "rows a point apart");
    assert_eq!(
        (full.x, full.width),
        (20.0, 362.0),
        "a full-width row is its card's width, 20 in"
    );
    let value = frame("value");
    assert!(
        value.x + value.width <= 382.0 - 10.0 + 0.01,
        "its content ends 10 short of the card's edge: {value:?}"
    );
    let line = k.grouped_separator(id("first")).unwrap();
    assert_eq!(
        (line.rect.x, line.rect.y, line.rect.width, line.rect.height),
        (10.0, 36.0, 342.0, 1.0),
        "in the gap under the row, 10 in from both ends"
    );
    assert_eq!(
        line.color,
        ColorValue::LightDark(
            Color::parse_hex("#efefef").unwrap(),
            Color::parse_hex("#333333").unwrap()
        ),
        "the measured form's separator"
    );
    // A destructive row is the Mac's push button: its box at the row's
    // inset, the button's padding inside it, its height the button's.
    let delete = k.node(id("delete")).unwrap();
    assert_eq!(delete.frame.x, 30.0, "10 into the card");
    assert_eq!(delete.frame.width, 342.0);
    assert_eq!(k.resolved_padding(delete.key).unwrap().0, 12.0);
    let line = k.grouped_separator(id("delete")).unwrap().rect;
    assert_eq!(
        (line.x, line.y, line.width),
        (0.0, delete.frame.height, 342.0),
        "its line spans the button's box, 10 in from both ends, in the gap"
    );
    // A `delete=` row's swipe clips what it holds: its line is its own foot.
    let swipe = k
        .node(k.node(id("swiped")).unwrap().parent.unwrap())
        .unwrap();
    let line = k.grouped_separator(swipe.id).unwrap().rect;
    assert_eq!(
        line.y,
        swipe.frame.height - 1.0,
        "inside the swipe that clips"
    );
    // A native button as wide as its row ends with the card, as an unsized
    // one at its own size may: no wider than the card less its inset.
    let wide = frame("native-full");
    assert_eq!((wide.x, wide.width), (30.0, 352.0));
    // A destructive one a class made native is inset by its push button's
    // padding, and ends with the card too.
    let pushed = frame("native-pushed");
    assert_eq!(pushed.x + pushed.width, 382.0, "{pushed:?}");
    // A swipe written by hand, as wide as its card: 10 in, as the
    // expansion's, and ending with the card.
    let written = frame("written");
    assert_eq!(
        (written.x, written.x + written.width),
        (30.0, 382.0),
        "{written:?}"
    );
    // The card, the rows' box, has its corners on SwiftUI's continuous
    // curve, not a circle's.
    let card = k
        .node(k.node(id("first")).unwrap().parent.unwrap())
        .unwrap();
    assert_eq!(
        card.style.border_radius_top_left,
        exact_kernel::Dimension::Points(10.0)
    );
    assert!(
        card.style.rare.corner_shape.is_apple_continuous(),
        "{:?}",
        card.style.rare.corner_shape
    );
    set_column(Column::Web);
}
