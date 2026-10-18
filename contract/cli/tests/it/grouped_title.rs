//! LLP 1116.021 D4: in a grouped section, a `button` or `link` that holds
//! the row's title, with a later text beside it, is the row's title: it
//! grows, its face is leading and centred in the row's height, and the later
//! text is the value at the far edge (round 19's grocery-list). A lone button
//! row stays an action cell, and an authored `text-align` wins.

use exact_kernel::{Kernel, Offer, StyleMask, TextAlign};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(body: &str) -> Runner<NoData> {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    let source = format!(
        "component App\n  action go\n  action flip(v: bool)\n  view\n    column testId=\"root\" width=402 height=874\n{body}"
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

const ROWS: &str = r#"list appearance="auto" testId="list" flex=1
  section
    row testId="item" gap=8 align-items="center"
      input type="checkbox" checked=false change=flip aria-label="Bananas"
      button press=go testId="toggle" display="flex" min-height=44
        text "Bananas" testId="name"
      text "× 1" testId="qty"
    row testId="said" gap=8
      button press=go testId="said-toggle" text-align="center"
        text "Centred" testId="said-name"
      text "× 2"
    button press=go testId="action"
      text "Add Item"
    link href="/x" testId="link"
      text "Open""#;

#[test]
fn a_button_holding_the_title_is_the_title_and_the_later_text_its_value() {
    let r = boot(ROWS);
    let k = r.kernel();
    let node = |t: &str| k.node(id(&r, t)).unwrap();
    let (row, button, name, qty) = (
        node("item").frame,
        node("toggle").frame,
        node("name").frame,
        node("qty").frame,
    );
    assert!(
        row.x + row.width - (qty.x + qty.width) <= 17.0 && qty.width < row.width / 4.0,
        "the value trails: {qty:?} in {row:?}"
    );
    assert!(
        button.x - row.x < 30.0 && button.x + button.width >= qty.x - 9.0,
        "the title grows to the value: {button:?}, {qty:?}"
    );
    assert!(
        (name.y + name.height / 2.0 - button.y - button.height / 2.0).abs() <= 1.0,
        "the face is centred in the button's height: {name:?} in {button:?}"
    );
    let align = |t: &str| node(t).computed_style(StyleMask::ALL).text_align;
    assert!(
        matches!(align("name"), TextAlign::Start | TextAlign::Left),
        "the face is leading"
    );
    assert_eq!(align("said-name"), TextAlign::Center, "the author's wins");
    let list = k.grouped_list(id(&r, "list")).unwrap();
    let action = |t: &str| {
        let view = id(&r, t);
        list.sections[0]
            .rows
            .iter()
            .find(|row| row.view == view)
            .unwrap()
            .action
    };
    assert!(action("action"), "a lone button row stays an action cell");
    let qty_colour = node("qty").text_color();
    assert_ne!(
        qty_colour,
        node("name").text_color(),
        "the value is the secondary label"
    );
}

/// A row of columns its author sized (`flex=1` each, event-rsvp's counts
/// over captions, round 21) is laid out as CSS lays it: no column is
/// squeezed under its caption's width, so no caption runs into the next.
#[test]
fn columns_the_author_sized_keep_their_words_width() {
    let body = "list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    row padding-top=14 padding-bottom=14\n      column flex=1 align-items=\"center\" testId=\"c0\"\n        text \"1\"\n        text \"Going\" testId=\"t0\"\n      column flex=1 align-items=\"center\" testId=\"c1\"\n        text \"1\"\n        text \"Maybe\" testId=\"t1\"\n      column flex=1 align-items=\"center\" testId=\"c2\"\n        text \"0\"\n        text \"Can’t go\" testId=\"t2\"";
    for root in [17.0f32, 53.0] {
        let mut r = boot(body);
        r.set_root_font_size(root.into()).unwrap();
        let k = r.kernel_mut();
        let page = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
        k.compute_layout(page, Offer::definite(402.0, 874.0))
            .unwrap();
        let k = r.kernel();
        let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
        for (column, caption) in [("c0", "t0"), ("c1", "t1"), ("c2", "t2")] {
            let (c, t) = (frame(column), frame(caption));
            assert!(
                c.width >= t.width - 0.5,
                "{root}: {column} keeps {caption}'s width: {c:?} {t:?}"
            );
        }
        for (a, b) in [("c0", "c1"), ("c1", "c2")] {
            let (a, b) = (frame(a), frame(b));
            assert!(a.x + a.width <= b.x + 0.5, "{root}: {a:?} {b:?}");
        }
    }
}
