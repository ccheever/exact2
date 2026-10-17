//! A grouped list's custom rows start where the platform's list starts a
//! standard row's text: at the cell's layout margin. UIKit chooses its
//! margins (16 or 20 points, none beside the iPhone Duo's side strip), the
//! host reports them (`Kernel::set_grouped_margins`), and the sheet's
//! stand-ins give way to them where the author wrote nothing.

use exact_kernel::{Frame, GroupedMargins, GroupedRow, Kernel, Offer, Op};
use exact_runner::{DataError, DataSource, Event, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// `list` (indented under the view) in a 402-point column.
fn boot(list: &str) -> Runner<NoData> {
    let list = list
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    let source = format!(
        "component App\n  state gone = \"\"\n  action drop(id: string)\n    gone = id\n  view\n    column testId=\"root\" width=402 height=874\n{list}"
    );
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
    lay_out(&mut r);
    r
}

fn lay_out(r: &mut Runner<NoData>) {
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
}

fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn frame(r: &Runner<NoData>, test_id: &str) -> Frame {
    r.kernel().node(id(r, test_id)).unwrap().frame
}

fn report(r: &mut Runner<NoData>, margins: Option<GroupedMargins>) {
    let list = id(r, "list");
    r.kernel_mut().set_grouped_margins(list, margins).unwrap();
    lay_out(r);
}

/// Where each row's content starts and ends: its first and last parts.
fn content(r: &Runner<NoData>, first: &str, last: &str) -> (f32, f32) {
    let (a, b) = (frame(r, first), frame(r, last));
    (a.x, b.x + b.width)
}

/// The left and right edges of the box `test_id` or, with `up`, of the box
/// that holds it.
fn edges(r: &Runner<NoData>, test_id: &str, up: bool) -> (f32, f32) {
    let k = r.kernel();
    let node = k.node(id(r, test_id)).unwrap();
    let node = if up {
        k.node(node.parent.unwrap()).unwrap()
    } else {
        node
    };
    (node.frame.x, node.frame.x + node.frame.width)
}

const ROWS: &str = "list appearance=\"auto\" listStyle=\"STYLE\" testId=\"list\" flex=1
  section
    row testId=\"plain\"
      text \"Standard\"
    row testId=\"custom\"
      view width=28 height=28 flex-shrink=0 testId=\"box\"
      text \"Custom\" flex=1 testId=\"custom-end\"
    row testId=\"symbol\"
      image \"symbol:sf/star\" testId=\"star\"
      column flex=1 min-width=0 testId=\"lines\"
        text \"One\"
        text \"Two\"
        text \"Three\"
    row delete=drop(\"d\") testId=\"swipe\"
      column flex=1 min-width=0 testId=\"swiped\"
        text \"One\"
        text \"Two\"
        text \"Three\"
    row margin-left=0 padding-right=4 testId=\"own\"
      view width=28 height=28 flex-shrink=0 testId=\"own-box\"
      text \"Flush\" flex=1 testId=\"own-end\"
    button appearance=\"auto\" press=drop(\"n\") testId=\"native\"
      text \"Go\"
    if gone == \"n\"
      row testId=\"late\"
        view width=28 height=28 flex-shrink=0 testId=\"late-box\"
        text \"Late\" flex=1 testId=\"late-end\"
      row delete=drop(\"z\") testId=\"late-swipe\"
        text \"Late swipe\" flex=1 testId=\"late-swiped\"
    scroll swipeContent=\"hand\" swipeTrailing=\"pin\" display=\"flex\" overflow-x=\"scroll\" overflow-y=\"hidden\" scroll-snap-type=\"x mandatory\" testId=\"hand-scroll\"
      row id=\"hand\" width=\"100%\" flex-shrink=0 scroll-snap-align=\"start\" testId=\"hand\"
        text \"By hand\" flex=1 testId=\"hand-text\"
      button id=\"pin\" press=drop(\"p\") flex-shrink=0 scroll-snap-align=\"start\"
        text \"Pin\"
  if gone == \"n\"
    section
      row testId=\"late-section\"
        view width=28 height=28 flex-shrink=0 testId=\"late-section-box\"
        text \"Late section\" flex=1 testId=\"late-section-end\"
";

#[test]
fn custom_rows_follow_the_platforms_cards_and_cell_margins() {
    let mut r = boot(&ROWS.replace("STYLE", "inset-grouped"));
    // The sheet's stand-ins: a card 16 in from each edge, content 16 into it.
    assert_eq!(content(&r, "box", "custom-end"), (32.0, 370.0));
    // The iPhone Duo's inner panel: a card 20 in, ending at the side strip,
    // and cells with 20-point margins.
    let duo = GroupedMargins {
        card: [20.0, 0.0],
        cell: [20.0, 20.0],
    };
    let native = edges(&r, "native", false);
    report(&mut r, Some(duo));
    assert_eq!(
        content(&r, "box", "custom-end"),
        (40.0, 382.0),
        "the card's 20 and the cell's 20 at the leading edge, 20 short of the trailing edge"
    );
    assert_eq!(
        edges(&r, "custom", false).1,
        402.0,
        "the row ends with its card, its inset inside it"
    );
    let (star, lines) = (frame(&r, "star"), frame(&r, "lines"));
    assert_eq!(
        (lines.x, star.x + star.width / 2.0 < lines.x),
        (80.0, true),
        "after a symbol, the text 40 past the margin, as UIKit's symbol cell sets it"
    );
    assert_eq!(
        content(&r, "swiped", "swiped"),
        (40.0, 382.0),
        "a swipe row's inset is inside it, and moves with the margin"
    );
    assert_eq!(
        edges(&r, "swipe", true),
        (20.0, 402.0),
        "and its scroll spans the card, so the whole cell swipes and presses"
    );
    assert_eq!(
        (native, edges(&r, "native", false)),
        ((32.0, 386.0), (40.0, 402.0)),
        "a native button row starts at the margin and keeps its own padding, so it still ends with the card"
    );
    assert_eq!(
        content(&r, "own-box", "own-end"),
        (20.0, 398.0),
        "an end the author wrote stays the author's"
    );
    assert_eq!(
        (content(&r, "hand-text", "hand-text"), edges(&r, "hand", false).1),
        ((40.0, 382.0), 382.0),
        "a swipe written by hand is styled as any row: its insets on its scroll, its content within it"
    );
    r.dispatch(id(&r, "native"), Event::Press).unwrap();
    lay_out(&mut r);
    assert_eq!(
        content(&r, "late-box", "late-end"),
        (40.0, 382.0),
        "a row that comes after the report takes the margins too"
    );
    assert_eq!(
        content(&r, "late-swiped", "late-swiped"),
        (40.0, 382.0),
        "and a swipe row's content"
    );
    assert_eq!(
        content(&r, "late-section-box", "late-section-end"),
        (40.0, 382.0),
        "and a section's"
    );
    // A row moved out of the list into the page sheds the margins with it.
    let (root, custom) = (id(&r, "root"), id(&r, "custom"));
    let mut children = r.kernel().node(root).unwrap().children();
    children.push(custom);
    r.kernel_mut()
        .apply(0, 1_000_000, &[Op::SetChildren { id: root, children }])
        .unwrap();
    lay_out(&mut r);
    assert_eq!(
        content(&r, "box", "custom-end"),
        (16.0, 386.0),
        "in the page, the sheet's 16 at each end again"
    );
    // A section and one of its rows, moved into the page at once, both shed
    // them: the row's group no longer reaches the list when the row moves.
    let row = id(&r, "late-section");
    let section = {
        let k = r.kernel();
        let group = k.node(row).unwrap().parent.unwrap();
        k.node(group).unwrap().parent.unwrap()
    };
    let mut children = r.kernel().node(root).unwrap().children();
    children.extend([section, row]);
    r.kernel_mut()
        .apply(0, 1_000_001, &[Op::SetChildren { id: root, children }])
        .unwrap();
    lay_out(&mut r);
    assert_eq!(
        content(&r, "late-section-box", "late-section-end"),
        (16.0, 386.0)
    );
    report(&mut r, None);
    assert_eq!(
        content(&r, "own-box", "own-end"),
        (16.0, 382.0),
        "cleared, the sheet's again"
    );
}

#[test]
fn a_plain_lists_rows_follow_its_cells_margins() {
    let mut r = boot(&ROWS.replace("STYLE", "plain"));
    assert_eq!(content(&r, "box", "custom-end"), (16.0, 386.0));
    // Beside the Duo's side strip a plain list's cells have no trailing margin.
    report(
        &mut r,
        Some(GroupedMargins {
            card: [0.0, 0.0],
            cell: [20.0, 0.0],
        }),
    );
    assert_eq!(content(&r, "box", "custom-end"), (20.0, 402.0));
    assert_eq!(
        edges(&r, "custom", false).1,
        402.0,
        "the row ends at the list's edge, not past it"
    );
}

#[test]
fn a_margin_that_is_no_length_is_refused() {
    let mut r = boot(&ROWS.replace("STYLE", "inset-grouped"));
    let list = id(&r, "list");
    for bad in [f32::NAN, -1.0, f32::INFINITY] {
        let margins = GroupedMargins {
            card: [bad, 0.0],
            cell: [16.0, 16.0],
        };
        assert!(r
            .kernel_mut()
            .set_grouped_margins(list, Some(margins))
            .is_err());
    }
}

/// A bare `text` in a section is a row (docs/recipes/navigation-stack.md's
/// ingredients): it is the platform's cell, its title the text, as a `row`
/// holding the text is, and not a custom row whose lines sat at the top of
/// the cell.
#[test]
fn a_bare_text_row_is_a_cells_title() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    text \"200 g spaghetti\" testId=\"bare\"\n    row testId=\"held\"\n      text \"200 g spaghetti\"");
    let rows = &r.kernel().grouped_list(id(&r, "list")).unwrap().sections[0].rows;
    assert_eq!(rows[0].title.as_deref(), Some("200 g spaghetti"));
    assert!(!rows[0].custom);
    assert_eq!(
        GroupedRow {
            view: rows[1].view,
            ..rows[0].clone()
        },
        rows[1],
        "the same cell as a row holding the text"
    );
}
