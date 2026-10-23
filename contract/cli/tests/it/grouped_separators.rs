//! LLP 1084 D7: a grouped list's row spans its card, `border-box`, with
//! the platform's insets as its padding, and the separator under it is
//! decoration the kernel places (`Kernel::grouped_separator`).

use exact_kernel::{Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// `body` under a 402 × 874 column.
fn boot(body: &str) -> Runner<NoData> {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    let source =
        format!("style Native\n  appearance=\"auto\"\ncomponent App\n  action go\n  view\n    column testId=\"root\" width=402 height=874\n{body}");
    let plan = contract::bake(
        contract::compile(&source).unwrap_or_else(|e| panic!("{e}")),
        NoData,
    )
    .unwrap();
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
    r
}

fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// LLP 1084 D7: a row as wide as its card stays inside it, its insets
/// inside its box, and the separator under each row is the platform's
/// decoration: from the row's content edge to the card's trailing edge, at
/// its foot, in the platform's colour, not under the card's last row nor
/// where the author wrote that edge.
#[test]
fn a_full_width_row_stays_in_its_card_and_its_separator_is_decoration() {
    let r = boot(
        "list appearance=\"auto\" testId=\"list\" flex=1
  section
    row width=\"100%\" testId=\"full\"
      image \"symbol:sf/star\"
      text \"Custom\" flex=1
      text \"42\" testId=\"value\"
    row width=\"100%\" padding-left=\"10%\" testId=\"percent\"
      text \"Percent\"
    row border-bottom-width=2 border-bottom-style=\"solid\" testId=\"own\"
      text \"Own edge\"
    row testId=\"before-top\"
      text \"Before a top edge\"
    row border-top-width=0 testId=\"top\"
      text \"Top\"
    row border-bottom-color=\"red\" testId=\"colour\"
      text \"Colour\"
    row testId=\"last\"
      text \"Last\"",
    );
    let k = r.kernel();
    let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
    let full = frame("full");
    assert_eq!((full.x, full.width), (16.0, 370.0), "inside the card");
    let value = frame("value");
    assert!(
        value.x + value.width <= 386.0 - 16.0 + 0.01,
        "the value ends at the trailing inset: {value:?}"
    );
    let line = |t: &str| k.grouped_separator(id(&r, t)).map(|s| s.rect);
    let full_line = line("full").unwrap();
    assert_eq!(
        (full_line.x, full_line.y, full_line.width, full_line.height),
        (55.0, 51.0, 315.0, 1.0),
        "from the text after the symbol to the card's edge, at the row's foot"
    );
    // A percentage padding is of the card's width (CSS Box Model 3 §5.2).
    assert_eq!(line("percent").unwrap().x, 37.0);
    assert_eq!(line("own"), None, "the author's bottom edge replaces it");
    assert_eq!(line("before-top"), None, "and the next row's top edge");
    assert!(line("colour").is_some(), "a colour alone recolours nothing");
    assert_eq!(line("last"), None, "the card's last row has none");
    assert_eq!(
        k.grouped_separator(id(&r, "full")).unwrap().color,
        exact_kernel::ColorValue::Role(
            exact_kernel::style::roles::role("-exact-separator").unwrap()
        ),
        "the platform's colour"
    );
    let rows = &k.grouped_list(id(&r, "list")).unwrap().sections[0].rows;
    let owned: Vec<bool> = rows.iter().map(|row| row.authored_edge).collect();
    assert_eq!(owned, [false, false, true, false, false, false, false]);
    let next: Vec<bool> = rows.iter().map(|row| row.next_authored_top).collect();
    assert_eq!(next, [false, false, false, true, false, false, false]);
}

/// The row after a row is the next one shown, as UIKit's list has it: a
/// hidden row is no neighbour, and a row followed only by hidden ones is
/// the card's last. A row that is shown but invisible draws none. A
/// `delete=` row's edges are its content's, which its author wrote.
#[test]
fn a_rows_neighbour_is_the_next_shown_and_a_swipe_rows_edges_are_its_contents() {
    let r = boot(
        "list appearance=\"auto\" flex=1
  section
    row testId=\"a\"
      text \"A\"
    row display=\"none\" border-top-width=0
      text \"Hidden\"
    row testId=\"b\"
      text \"B\"
    row visibility=\"hidden\" testId=\"invisible\"
      text \"Invisible\"
    row delete=go border-bottom-style=\"none\" testId=\"swiped\"
      text \"Swiped\"
    row delete=go border-top-width=0 testId=\"next\"
      text \"Next\"
    row testId=\"shown-last\"
      text \"Shown last\"
    row display=\"none\"
      text \"Hidden last\"",
    );
    let k = r.kernel();
    let line = |t: &str| k.grouped_separator(id(&r, t));
    assert!(
        line("a").is_some(),
        "a hidden row's top edge is not the next row's"
    );
    assert!(line("b").is_some());
    assert_eq!(line("invisible"), None, "an invisible row draws none");
    let scroll = |t: &str| k.node(id(&r, t)).unwrap().parent.unwrap();
    assert_eq!(
        k.grouped_separator(scroll("swiped")),
        None,
        "the content's own `none`"
    );
    assert_eq!(line("shown-last"), None, "the last row shown has none");
}

/// A native button row keeps its inset as a margin and is no wider than its
/// card less its insets: one its author makes as wide as the row stays in
/// the card, and an unsized one is as wide as it was.
#[test]
fn a_full_width_native_button_row_stays_in_its_card() {
    let r = boot(
        "list appearance=\"auto\" flex=1
  section
    button appearance=\"auto\" press=go width=\"100%\" testId=\"full\"
      text \"Full\"
    button appearance=\"auto\" press=go testId=\"unsized\"
      text \"Unsized\"
    button appearance=\"auto\" press=go width=\"100%\" max-width=\"none\" testId=\"own\"
      text \"Own\"
    button appearance=\"auto\" press=go margin-left=0 testId=\"placed\"
      text \"Placed\"
    button class=Native press=go width=\"100%\" testId=\"class-symbol\"
      image \"symbol:sf/star\"
      text \"Starred\"",
    );
    let k = r.kernel();
    let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
    let (full, unsized_) = (frame("full"), frame("unsized"));
    assert_eq!(
        (full.x, full.x + full.width),
        (32.0, 386.0),
        "inside the card"
    );
    assert_eq!(
        (unsized_.x, unsized_.width),
        (32.0, 354.0),
        "stretched, as before"
    );
    assert_eq!(frame("own").width, 370.0, "an authored max-width wins");
    assert_eq!(
        (frame("placed").x, frame("placed").width),
        (16.0, 370.0),
        "a row its author placed with a margin is the author's, stretched across the card"
    );
    // A row a class made one: its inset knows a leading symbol, and its
    // widest does too.
    let symbol = frame("class-symbol");
    // At the test's 16-point root UIKit's text starts 55 in (`-exact-list-symbol-inset`).
    assert_eq!((symbol.x, symbol.x + symbol.width), (71.0, 385.0));
}
