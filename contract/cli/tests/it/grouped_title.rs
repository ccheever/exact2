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
    let inset = k.resolved_padding(node("item").key).unwrap().0;
    assert!(
        button.x - (row.x + inset) < 30.0 && button.x + button.width >= qty.x - 9.0,
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

/// A `column` around the title's one text is the title as a bare text is:
/// it grows, and the text after it is the value at the far edge, not a
/// second title sharing the row (grocery-list's quantity mid-row, round 24).
/// Where the two do not fit one line the value goes under the title, at its
/// leading edge, and the title takes the row's width, as UIKit's value cell
/// stacks them (iOS 27): a long title at the default size, both at the
/// largest. The row is a custom cell, so a word longer than the title's line
/// breaks at a letter, as a UILabel's does, and not between syllables (LLP
/// 1141.001 D5).
#[test]
fn a_column_holding_the_title_is_the_title_and_the_later_text_its_value() {
    for (name, root, beside) in [
        ("Tomatoes", 17.0f32, true),
        ("Extraordinarily long heirloom tomatoes", 17.0, false),
        ("Tomatoes", 53.0, false),
        ("Extraordinarily long heirloom tomatoes", 53.0, false),
    ] {
        let body = format!("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    row press=go testId=\"item\" gap=10 align-items=\"center\"\n      input type=\"checkbox\" checked=false change=flip aria-label=\"Tomatoes\"\n      column flex=1 min-width=0 testId=\"title\"\n        text \"{name}\" testId=\"name\"\n      text \"× 6\" testId=\"qty\"");
        let mut r = boot(&body);
        r.set_root_font_size(root.into()).unwrap();
        let k = r.kernel_mut();
        let page = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
        k.compute_layout(page, Offer::definite(402.0, 874.0))
            .unwrap();
        let k = r.kernel();
        let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
        let (row, title, qty) = (frame("item"), frame("title"), frame("qty"));
        // The value's box is as wide as its words: grown, its words sat at
        // its leading edge, mid-row.
        assert!(
            qty.width <= 3.0 * root,
            "{name} at {root}: the value keeps its words' width: {qty:?}"
        );
        if beside {
            assert!(
                title.x + title.width <= qty.x && qty.y < title.y + title.height,
                "{name} at {root}: the value is beside the title: {title:?} {qty:?}"
            );
        } else {
            assert!(
                qty.x == title.x && qty.y >= title.y + title.height && title.width >= 0.7 * row.width,
                "{name} at {root}: the value is under the title, which has the row: {title:?} {qty:?} in {row:?}"
            );
        }
        let style = k
            .node(id(&r, "name"))
            .unwrap()
            .computed_style(StyleMask::ALL);
        assert_eq!(
            (style.hyphens, style.overflow_wrap),
            (
                exact_kernel::Hyphens::Manual,
                exact_kernel::OverflowWrap::BreakWord
            ),
            "a long word breaks at a letter"
        );
    }
}

/// LLP 1141.001 D5: the standard cell's texts hyphenate, as
/// UIListContentConfiguration's labels do (a title, a title and its value,
/// the subtitle cell, `RowText`'s stand-in for it, a swipe's row of that
/// shape). An author's own content in a row (Hackers' feed row: a thumbnail
/// button beside a stack of a domain, a title and a line of counts) is a
/// custom cell's, whose labels do not: CSS's `manual`. Every row's long word
/// still breaks rather than running past its line.
#[test]
fn only_the_standard_cells_texts_hyphenate() {
    use exact_kernel::{Hyphens, OverflowWrap};
    let rows = r#"list appearance="auto" listStyle="plain" testId="list" flex=1
  section
    row
      text "Title" testId="title"
    row
      image "symbol:star"
      text "Title" testId="valued"
      text "Value" testId="value"
      image "symbol:chevron.forward"
    row
      column
        text "Title" testId="stacked"
        text "Subtitle" testId="subtitle"
    row delete=go
      text "Swiped" testId="swiped"
    row press=go gap=12
      button press=go aria-label="Open link" width=55 height=55
        image "symbol:sf/safari"
      column flex=1 min-width=0 gap=6
        text "example.com" testId="domain"
        text "LLMs are making formal software verification feasible" testId="post"
        row gap=8
          text "↑ 12" testId="score"
    row
      text "A" testId="first"
      text "B"
      text "C""#;
    let r = boot(rows);
    let k = r.kernel();
    let style = |t: &str| {
        let s = k.node(id(&r, t)).unwrap().computed_style(StyleMask::ALL);
        (s.hyphens, s.overflow_wrap)
    };
    for t in ["title", "valued", "value", "stacked", "subtitle", "swiped"] {
        assert_eq!(style(t), (Hyphens::Auto, OverflowWrap::BreakWord), "{t}");
    }
    for t in ["domain", "post", "score", "first"] {
        assert_eq!(style(t), (Hyphens::Manual, OverflowWrap::BreakWord), "{t}");
    }
}

/// A swipe's row is carried as a custom row, so its bare title and value
/// stack as the value cell's do where they do not fit (round 26's
/// grocery-list: "heirloo/m" beside its quantity at the largest size). A row
/// of two bare texts with no swipe is the value cell the platform's list
/// draws, which stacks them itself: the sheet keeps them side by side.
#[test]
fn a_swipe_rows_bare_title_and_value_stack_where_they_do_not_fit() {
    for (swipe, under) in [(" delete=go", true), ("", false)] {
        let body = format!("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    row press=go{swipe} testId=\"item\"\n      image \"symbol:circle\"\n      text \"Tomatoes\" flex=1 min-width=0 testId=\"name\"\n      text \"× 6\" testId=\"qty\"");
        let mut r = boot(&body);
        r.set_root_font_size(53.0).unwrap();
        let k = r.kernel_mut();
        let page = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
        k.compute_layout(page, Offer::definite(402.0, 874.0))
            .unwrap();
        let k = r.kernel();
        let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
        let (name, qty) = (frame("name"), frame("qty"));
        assert_eq!(
            qty.y >= name.y + name.height,
            under,
            "{swipe:?}: {name:?} {qty:?}"
        );
    }
}

/// A text and the worded button that ends its row stay side by side while
/// the text keeps room for its words, and at the accessibility sizes the
/// button goes under the text, as a hand-built cell switches its stack to
/// vertical (household-list at the largest size: its message beside Try
/// Again broke inside every word). A symbol-only button stays beside it.
#[test]
fn a_rows_trailing_button_goes_under_its_text_where_the_text_would_be_squeezed() {
    for (button, root, under) in [
        ("text \"Try Again\"", 17.0f32, false),
        ("text \"Try Again\"", 23.0, false),
        ("text \"Try Again\"", 40.0, true),
        ("text \"Try Again\"", 53.0, true),
        (
            "image \"symbol:refresh\" aria-label=\"Try Again\"",
            53.0,
            false,
        ),
    ] {
        let body = format!("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    row testId=\"item\" gap=8\n      text \"Can't reach the server. Check that it's running, then try again.\" flex=1 min-width=0 testId=\"message\"\n      button press=go testId=\"retry\"\n        {button}");
        let mut r = boot(&body);
        r.set_root_font_size(root.into()).unwrap();
        let k = r.kernel_mut();
        let page = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
        k.compute_layout(page, Offer::definite(402.0, 874.0))
            .unwrap();
        let k = r.kernel();
        let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
        let (row, message, retry) = (frame("item"), frame("message"), frame("retry"));
        if under {
            assert!(
                retry.y >= message.y + message.height && message.width >= 0.7 * row.width,
                "{button} at {root}: the button is under the text, which has the row: {message:?} {retry:?} in {row:?}"
            );
        } else {
            assert!(
                message.x + message.width <= retry.x && retry.y < message.y + message.height,
                "{button} at {root}: the button is beside the text: {message:?} {retry:?}"
            );
        }
    }
}

/// A row whose one part is a box of the author's (a button's face laid out
/// in a `row`) fills the cell, as a hand-built cell pins its stack to its
/// content view's edges: a bar in it is a share of the row's width, not of
/// its label's (round 29's polls: two 50% bars of different lengths).
#[test]
fn a_rows_one_box_fills_the_row() {
    let option = |n: &str, label: &str, mark: &str| {
        format!("    button press=go testId=\"{n}\"\n      row gap=8 align-items=\"center\" testId=\"{n}-face\"\n        {mark}\n        column flex=1 min-width=0\n          text \"{label}\"\n          view height=8 testId=\"{n}-track\"\n            view width=\"50%\" height=8 testId=\"{n}-bar\"\n")
    };
    let body = format!(
        "list appearance=\"auto\" testId=\"list\" flex=1\n  section\n{}{}",
        option("a", "Tacos", "image \"symbol:checkmark\""),
        option(
            "b",
            "A much longer option label",
            "if false\n          image \"symbol:checkmark\""
        )
    );
    let r = boot(&body);
    let k = r.kernel();
    let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
    for n in ["a", "b"] {
        let (row, face) = (frame(n), frame(&format!("{n}-face")));
        assert!(
            face.width >= row.width - 40.0,
            "{n}: the face fills its row: {face:?} in {row:?}"
        );
    }
    let (a, b) = (frame("a-bar"), frame("b-bar"));
    assert!(
        (a.width - frame("a-track").width / 2.0).abs() < 0.5
            && (frame("b-face").width - frame("a-face").width).abs() < 0.5,
        "equal shares draw bars a share of the row: {a:?} {b:?}"
    );
}

/// A bare title before two values (a lap, its time and its total) is no
/// value cell: it is boxed with them, and each value wraps under the one
/// before where they do not fit, as a hand-built cell's labels stack at the
/// accessibility sizes. Beside the two at the largest size the title was
/// squeezed to no width and the total ran past the row (stopwatch, round 30).
#[test]
fn a_bare_title_before_two_values_keeps_its_words_and_they_stack() {
    for (root, beside) in [(17.0f32, true), (53.0, false)] {
        let body = "list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    row testId=\"item\" gap=8 align-items=\"center\"\n      text \"Lap 12\" flex=1 testId=\"title\"\n      text \"00:13.51\" font-variant-numeric=\"tabular-nums\" testId=\"lap\"\n      text \"03:06.12\" font-variant-numeric=\"tabular-nums\" testId=\"total\"";
        let mut r = boot(body);
        r.set_root_font_size(root.into()).unwrap();
        let k = r.kernel_mut();
        let page = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
        k.compute_layout(page, Offer::definite(402.0, 874.0))
            .unwrap();
        let k = r.kernel();
        let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
        let (row, title, lap, total) =
            (frame("item"), frame("title"), frame("lap"), frame("total"));
        assert!(
            title.width >= 2.0 * root,
            "at {root}: the title keeps its words: {title:?}"
        );
        for v in [lap, total] {
            assert!(
                v.x + v.width <= row.width + 0.5,
                "at {root}: the value stays in the row: {v:?} in {row:?}"
            );
        }
        if beside {
            assert!(
                title.x + title.width <= lap.x && lap.x + lap.width <= total.x && (lap.y - title.y).abs() < root,
                "at {root}: one line, the values at the far edge: {title:?} {lap:?} {total:?} in {row:?}"
            );
        } else {
            assert!(
                lap.y >= title.y + title.height - 0.5 && total.y >= lap.y + lap.height - 0.5 && lap.x == title.x && total.x == title.x,
                "at {root}: each value under the one before, at the title's edge: {title:?} {lap:?} {total:?}"
            );
        }
    }
}

/// A box its author grows before any text is the title's room, and a text
/// after it, shown or not by an `if`, the value at the far edge:
/// water-tracker's day row (round 33), whose "Goal met" took half the row
/// from its bar as a second title.
#[test]
fn a_grown_box_before_the_text_keeps_the_room_and_the_text_trails() {
    let r = boot(
        r##"list appearance="auto" testId="list" flex=1
  section
    row testId="day" gap=8 align-items="center"
      column flex=1 min-width=0 testId="bar"
        column gap=6 role="meter" aria-label="Yesterday" aria-valuenow="1"
          row flex-wrap="wrap" column-gap=8
            text "Yesterday" flex-grow=1 min-width=0
            text "1,200 ml" min-width=0
          view height=8 background-color="#ccc"
      if true
        text "Goal met" testId="met""##,
    );
    let k = r.kernel();
    let node = |t: &str| k.node(id(&r, t)).unwrap();
    let (row, bar, met) = (node("day").frame, node("bar").frame, node("met").frame);
    assert!(
        row.width - (met.x + met.width) <= 17.0 && met.width < row.width / 4.0,
        "the value trails: {met:?} in {row:?}"
    );
    assert!(
        bar.width > row.width / 2.0,
        "the bar keeps the room: {bar:?} in {row:?}"
    );
}
