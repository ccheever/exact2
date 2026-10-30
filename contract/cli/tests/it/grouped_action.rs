//! LLP 1116.019 D6: a grouped section's `button` row of a title, a symbol
//! before it at most and nothing after it, is an action cell, in the accent
//! on the sheet and in the kernel's model, however its symbol is given: a
//! literal, a template a prop names (a library row's) or an `if` that shows
//! it (theme2 16:37, stage 13 C). A row that says whether it is selected
//! is a choice, never one.

use exact_kernel::{Accessory, ColorValue, GroupedRow, Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(source: &str) -> Runner<NoData> {
    let plan = contract::bake(
        contract::compile(source).unwrap_or_else(|e| panic!("{e}")),
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
    layout(&mut r);
    r
}

fn layout(r: &mut Runner<NoData>) {
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
}

fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The kernel's model of the row `test_id` names.
fn row(r: &Runner<NoData>, test_id: &str) -> GroupedRow {
    let view = id(r, test_id);
    let list = r.kernel().grouped_list(id(r, "list")).unwrap();
    list.sections[0]
        .rows
        .iter()
        .find(|row| row.view == view)
        .unwrap()
        .clone()
}

fn colour(r: &Runner<NoData>, test_id: &str) -> ColorValue {
    r.kernel().node(id(r, test_id)).unwrap().text_color()
}

const SYMBOLS: &str = r#"component App
  state on = true
  action go
    on = ! on
  view
    column testId="root" width=402 height=874
      list appearance="auto" testId="list" flex=1
        section
          button press=go testId="written"
            image "symbol:add" testId="written-icon"
            text "Add" testId="written-title"
          Named(s="add", t="named", go=go)
          button press=go testId="shown"
            if on
              image "symbol:add" testId="shown-icon"
            text "Add" testId="shown-title"
          Both(s="add", t="both", on=on, go=go)
          button press=go testId="push"
            image `symbol:${"add"}`
            text "Open" testId="push-title"
            image "symbol:forward-chevron"
          button press=go testId="worded"
            if on
              text "New"
            text "Add" testId="worded-title"
          button press=go testId="pair"
            if on
              image "symbol:add"
              image "symbol:star"
            text "Add" testId="pair-title"
          button press=go testId="split"
            if on
              image "symbol:add"
            else
              text "New"
            text "Add" testId="split-title"
          button press=go testId="chosen"
            if on
              image "symbol:add" testId="chosen-icon"
            else if ! on
              image "symbol:star"
            text "Add" testId="chosen-title"
      text "accent" color="AccentColor" testId="accent"

component Named
  props
    s: string
    t: string
    go: action
  view
    button press=go testId=t
      image `symbol:${s}` testId=`${t}-icon`
      text "Add" testId=`${t}-title`

component Both
  props
    s: string
    t: string
    on: bool
    go: action
  view
    button press=go testId=t
      if on
        image `symbol:${s}` testId=`${t}-icon`
      text "Add" testId=`${t}-title`
"#;

#[test]
fn a_rows_symbol_a_prop_names_or_an_if_shows_is_the_leading_symbol_of_an_action_cell() {
    let mut r = boot(SYMBOLS);
    let label = ColorValue::Role(exact_kernel::style::roles::role("-exact-label").unwrap());
    let accent = colour(&r, "accent");
    for t in ["written", "named", "shown", "both", "chosen"] {
        assert!(row(&r, t).action, "{t}: an action cell");
        assert_eq!(colour(&r, &format!("{t}-title")), accent, "{t}: its ink");
    }
    let k = r.kernel();
    // Where a box sits in its row, and its size, to a hundredth.
    let place = |t: &str, row: &str| {
        let (f, at) = (
            k.node(id(&r, t)).unwrap().frame,
            k.node(id(&r, row)).unwrap().frame,
        );
        [f.x - at.x, f.y - at.y, f.width, f.height].map(|v| (v * 100.0).round() / 100.0)
    };
    for t in ["named", "shown", "both", "chosen"] {
        for part in ["icon", "title"] {
            assert_eq!(
                place(&format!("{t}-{part}"), t),
                place(&format!("written-{part}"), "written"),
                "{t}: the leading symbol's metrics, the title after it"
            );
        }
    }
    for t in ["named", "shown", "both"] {
        assert_eq!(
            row(&r, t).symbol,
            row(&r, "written").symbol,
            "{t}: the symbol it names, not a stand-in"
        );
    }
    assert!(!row(&r, "push").action);
    assert_eq!(colour(&r, "push-title"), label, "a row that pushes");
    for t in ["worded", "pair"] {
        assert!(!row(&r, t).action, "{t}: an `if` that shows more");
        assert_ne!(colour(&r, &format!("{t}-title")), accent, "{t}");
    }
    // An `else` that shows a text: the sheet's ink is one for every state,
    // so it is no action cell's (the kernel reads the row as it shows now).
    assert_ne!(colour(&r, "split-title"), accent, "split");
    assert!(
        !row(&r, "split").action,
        "split: the kernel takes the sheet's word"
    );
    let shown = id(&r, "shown");
    r.dispatch(shown, exact_runner::Event::Press).unwrap();
    layout(&mut r);
    for t in ["shown", "both"] {
        let model = row(&r, t);
        assert_eq!(
            (model.symbol, model.action),
            (None, true),
            "{t}: no symbol now, a title alone"
        );
        assert_eq!(colour(&r, &format!("{t}-title")), accent);
    }
}

const CHOICES: &str = r#"component App
  state picked = false
  action pick
    picked = ! picked
  view
    column testId="root" width=402 height=874
      list appearance="auto" testId="list" flex=1
        section
          button press=pick aria-selected=picked testId="off"
            text "Small" testId="off-title"
            if picked
              image "symbol:checkmark"
          button press=pick aria-selected=false testId="bare"
            text "Medium" testId="bare-title"
          button press=pick aria-selected=true testId="on"
            text "Large" testId="on-title"
            image "symbol:checkmark"
          button press=pick aria-selected=true testId="unmarked"
            text "Huge" testId="unmarked-title"
          button press=pick testId="action"
            text "Update" testId="action-title"
          button press=pick testId="unsaid"
            text "Dark" testId="unsaid-title"
            if picked
              image "symbol:checkmark"
      text "accent" color="AccentColor" testId="accent"
"#;

/// An unpicked choice shows no checkmark, so it has an action cell's
/// shape: `aria-selected`, true or false, says it is a choice, on the sheet
/// and in the kernel, so a native list draws its title in the label's
/// colour, as a checkmark list does.
#[test]
fn a_row_that_says_it_is_selected_is_a_choice_not_an_action() {
    let mut r = boot(CHOICES);
    let label = ColorValue::Role(exact_kernel::style::roles::role("-exact-label").unwrap());
    for t in ["off", "bare", "on", "unmarked"] {
        assert!(!row(&r, t).action, "{t}: a choice");
        assert_eq!(colour(&r, &format!("{t}-title")), label, "{t}");
    }
    assert_eq!(row(&r, "off").accessory, Accessory::None);
    assert!(row(&r, "action").action, "a row that says nothing is one");
    // A checkmark row that does not say `aria-selected`: unpicked it shows
    // a title alone, but its source can show a mark, so the sheet says no
    // action, and the kernel takes its word (QUEUE, 2026-10-21).
    assert!(
        !row(&r, "unsaid").action,
        "an unpicked choice row is no action"
    );
    assert_eq!(colour(&r, "unsaid-title"), label);
    assert_eq!(colour(&r, "action-title"), colour(&r, "accent"));
    let off = id(&r, "off");
    r.dispatch(off, exact_runner::Event::Press).unwrap();
    layout(&mut r);
    let picked = row(&r, "off");
    assert_eq!(
        (picked.accessory, picked.action),
        (Accessory::Checkmark, false),
        "picked, the same row"
    );
    assert_eq!(colour(&r, "off-title"), label);
}

/// A leading symbol and the title after it follow UIKit's list cell as the
/// text size changes: the symbol grows with the text through the standard
/// sizes and returns to its smallest at the accessibility sizes, and the
/// title starts where UIKit's `textLayoutGuide` does (iOS 27.0).
#[test]
fn a_leading_symbols_slot_and_the_title_after_it_follow_the_text_size() {
    let mut r = boot(SYMBOLS);
    let place = |r: &Runner<NoData>, t: &str| {
        let k = r.kernel();
        let (f, row) = (
            k.node(id(r, t)).unwrap().frame,
            k.node(id(r, "written")).unwrap().frame,
        );
        ((f.x - row.x) * 100.0).round() / 100.0
    };
    let width = |r: &Runner<NoData>| {
        (r.kernel().node(id(r, "written-icon")).unwrap().frame.width * 100.0).round() / 100.0
    };
    for (body, title, symbol) in [
        (17.0, 56.0, 24.0),
        (23.0, 63.67, 32.11),
        (14.0, 52.67, 20.28),
        (53.0, 86.33, 54.33),
    ] {
        r.set_root_font_size(body).unwrap();
        layout(&mut r);
        assert_eq!(
            place(&r, "written-icon"),
            16.0,
            "{body}: the symbol at the margin"
        );
        assert_eq!(width(&r), symbol, "{body}: the symbol's slot");
        assert_eq!(
            place(&r, "written-title"),
            title,
            "{body}: the title's inset"
        );
    }
}
