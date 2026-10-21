//! LLP 1116.019 D6: a grouped section's `button` row of a title, a symbol
//! before it at most and nothing after it, is an action cell, in the accent
//! on the sheet and in the kernel's model, however its symbol is given: a
//! literal, a template a prop names (a library row's) or an `if` that shows
//! it (theme2 16:37, stage 13 C).

use exact_kernel::{ColorValue, GroupedRow, Kernel, Offer};
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
    on = not on
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
            else if not on
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
