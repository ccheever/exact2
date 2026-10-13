//! LLP 1084: `list appearance="auto"` is a grouped list. Contract checks its
//! sections and writes its look as a sheet the author's rows replace; the
//! kernel reads its sections and rows as a native list draws them. Bare row
//! fixtures exercise LLP 1104's default in the grouped-list context.

use exact_kernel::{
    Accessory, Color, ColorValue, GroupedRow, Kernel, Offer, PropId, StyleMask, TextFont, TextLook,
};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A component whose view is `body` under a 402 × 874 column.
fn app(body: &str) -> String {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    format!(
        "component App\n  state on = true\n  state dark = false\n  action go\n    dark = not dark\n  action flip(v: bool)\n    on = v\n  view\n    column testId=\"root\" width=402 height=874\n{body}"
    )
}

fn boot(body: &str) -> Runner<NoData> {
    let plan = contract::bake(
        contract::compile(&app(body)).unwrap_or_else(|e| panic!("{e}")),
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

fn refused(body: &str) -> String {
    let e = contract::compile(&app(body)).unwrap_err();
    assert_eq!(e.id, "lower-grouped-list", "{}", e.message);
    e.message
}

const SETTINGS: &str = "list appearance=\"auto\" testId=\"list\" flex=1
  section testId=\"s0\"
    header
      text \"Account\"
    button press=go testId=\"profile\"
      image \"symbol:sf/person.circle\"
      text \"Profile\"
      image \"symbol:sf/chevron.right\"
    button press=go testId=\"notes\"
      image \"symbol:sf/bell\"
      text \"Notifications\"
      text \"On\"
      image \"symbol:forward-chevron\"
    button press=go testId=\"privacy\"
      column
        text \"Privacy\"
        text \"Screen lock\"
    footer
      text \"Who can see you.\"
  section testId=\"s1\"
    row testId=\"receipts\"
      text \"Read Receipts\"
      input type=\"checkbox\" switch checked=on input=flip testId=\"toggle\"
    button press=go testId=\"dark\"
      text \"Dark\"
      when dark
        image \"symbol:checkmark\"
    button press=go testId=\"custom\"
      image \"avatar.png\" width=40 height=40
      text \"Maya\"
  section
    button press=go destructive=true testId=\"delete\"
      text \"Delete Account\"
";

#[test]
fn the_kernel_reads_sections_and_rows_as_a_native_list_draws_them() {
    let r = boot(SETTINGS);
    let k = r.kernel();
    let list = k.grouped_list(id(&r, "list")).unwrap();
    assert_eq!(list.style, "inset-grouped", "the default style");
    assert_eq!(list.sections.len(), 3);
    let s0 = &list.sections[0];
    assert_eq!(s0.header.as_deref(), Some("Account"));
    assert_eq!(s0.footer.as_deref(), Some("Who can see you."));
    assert_eq!(
        s0.rows[0],
        GroupedRow {
            view: id(&r, "profile"),
            symbol: Some("person.circle".into()),
            title: Some("Profile".into()),
            accessory: Accessory::Disclosure,
            pressable: true,
            ..GroupedRow::default()
        }
    );
    let notes = &s0.rows[1];
    assert_eq!(
        (
            notes.symbol.as_deref(),
            notes.secondary.as_deref(),
            notes.subtitle,
            notes.accessory
        ),
        (Some("bell"), Some("On"), false, Accessory::Disclosure),
        "a role's chevron is a disclosure too"
    );
    let privacy = &s0.rows[2];
    assert_eq!(
        (
            privacy.title.as_deref(),
            privacy.secondary.as_deref(),
            privacy.subtitle
        ),
        (Some("Privacy"), Some("Screen lock"), true)
    );
    let s1 = &list.sections[1];
    assert_eq!((s1.header.clone(), s1.footer.clone()), (None, None));
    assert_eq!(s1.rows[0].accessory, Accessory::Toggle(id(&r, "toggle")));
    assert!(!s1.rows[0].pressable, "a `row` is not a button");
    assert_eq!(
        s1.rows[1].accessory,
        Accessory::None,
        "no checkmark until `dark`"
    );
    assert!(
        s1.rows[2].custom,
        "a raster leading image is the row's own content"
    );
    assert_eq!(s1.rows[2].title, None, "a custom row carries no parts");
    assert!(list.sections[2].rows[0].destructive);
    assert_eq!(
        k.grouped_list(id(&r, "s0")),
        None,
        "only a list with `listStyle`"
    );
}

/// LLP 1115.000 D13.2: a radio in a row is the row's accessory, before its
/// label as a web form writes it or after, with its bound `checked`; the
/// row is a standard cell of its texts, and it reads live.
#[test]
fn a_radio_is_its_rows_accessory() {
    let mut r = boot(
        r#"list appearance="auto" listStyle="inset-grouped" testId="list"
  section
    row
      text "Free" flex=1
      input type="radio" name="plan" value="free" checked=(not dark) testId="free"
    row
      input type="radio" name="plan" value="pro" checked=dark testId="pro"
      text "Pro"
      text "Monthly"
  section
    button press=go testId="dark"
      text "Dark""#,
    );
    let rows = |r: &Runner<NoData>| {
        let list = r.kernel().grouped_list(id(r, "list")).unwrap();
        list.sections[0].rows.clone()
    };
    let (free, pro) = (id(&r, "free"), id(&r, "pro"));
    let before = rows(&r);
    assert_eq!(before[0].accessory, Accessory::Radio(free, true));
    assert_eq!(before[1].accessory, Accessory::Radio(pro, false));
    assert!(!before[0].custom && !before[1].custom, "standard cells");
    assert_eq!(before[0].title.as_deref(), Some("Free"));
    assert_eq!(
        (before[1].title.as_deref(), before[1].secondary.as_deref()),
        (Some("Pro"), Some("Monthly")),
        "a leading radio leaves the row's texts its title and value"
    );
    assert!(
        !before[0].pressable,
        "the row is no button: its tap is the radio's"
    );
    let dark = id(&r, "dark");
    r.dispatch(dark, exact_runner::Event::Press).unwrap();
    let after = rows(&r);
    assert_eq!(after[0].accessory, Accessory::Radio(free, false));
    assert_eq!(after[1].accessory, Accessory::Radio(pro, true));
}

#[test]
fn the_rows_read_live() {
    let mut r = boot(SETTINGS);
    let dark = id(&r, "dark");
    r.dispatch(dark, exact_runner::Event::Press).unwrap();
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    assert_eq!(list.sections[1].rows[1].accessory, Accessory::Checkmark);
}

#[test]
fn the_sheet_draws_ios_metrics_and_the_author_replaces_it() {
    let r = boot(SETTINGS);
    let k = r.kernel();
    let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
    let list = k.node(id(&r, "list")).unwrap();
    assert_eq!(list.props.str(PropId::ListStyle), Some("inset-grouped"));
    // The header row is 10 + a line + 10; the rows start under it, 52 apart
    // (each draws its separator and overlaps the next by its width).
    let profile = frame("profile");
    assert_eq!(profile.height, 53.0, "52 and its 1-point separator");
    // (to a float's rounding: the header above is a line of the body style).
    assert!((frame("notes").y - profile.y - 52.0).abs() < 0.01);
    assert_eq!(
        profile.x, 72.0,
        "the row starts at the text, after an icon, as UIKit's label does"
    );
    let delete = frame("delete");
    assert_eq!(delete.x, 32.0, "no icon: 16 into the group");
    let own = boot(&SETTINGS.replace(
        "destructive=true testId=\"delete\"",
        "destructive=true testId=\"delete\" min-height=60",
    ));
    let k2 = own.kernel();
    assert_eq!(
        k2.node(id(&own, "delete")).unwrap().frame.height,
        61.0,
        "the author's row replaces the sheet's"
    );
}

#[test]
fn liststyle_names_the_style_and_is_checked() {
    let r = boot(&SETTINGS.replace(
        "appearance=\"auto\"",
        "appearance=\"auto\" listStyle=\"plain\"",
    ));
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    assert_eq!(list.style, "plain");
    assert!(
        refused("list appearance=\"auto\" listStyle=\"sidebar\"\n  section\n    text \"x\"")
            .contains("inset-grouped, grouped, plain")
    );
    assert!(refused("list listStyle=\"plain\"\n  text \"x\"").contains("appearance"));
    assert!(
        refused("list appearance=(on ? \"auto\" : \"none\")\n  text \"x\"").contains("literal")
    );
    assert!(refused("list appearance=\"auto\" virtualized=true estimated-item-height=52\n  section\n    text \"x\"").contains("virtualized"));
}

#[test]
fn a_grouped_list_holds_sections_and_a_section_its_texts_at_its_ends() {
    assert!(refused("list appearance=\"auto\"\n  text \"x\"").contains("`section`"));
    assert!(
        refused("list appearance=\"auto\"\n  when on\n    row\n      text \"x\"")
            .contains("`section`")
    );
    assert!(refused(
        "list appearance=\"auto\"\n  section\n    text \"x\"\n    header\n      text \"late\""
    )
    .contains("first"));
    assert!(refused("list appearance=\"auto\"\n  section\n    header\n      text \"a\"\n    header\n      text \"b\"").contains("at most one"));
    // A plain `list` is untouched.
    let r = boot("list testId=\"plain\" flex=1\n  text \"x\"");
    assert_eq!(r.kernel().grouped_list(id(&r, "plain")), None);
}

#[test]
fn a_class_replaces_the_sheet_as_an_attribute_does() {
    let src = app("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go class=Tall testId=\"tall\"\n      text \"Tall\"");
    let src = format!("style Tall\n  min-height=80\n{src}");
    let plan = contract::bake(contract::compile(&src).unwrap(), NoData).unwrap();
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
    assert_eq!(
        k.node_by_key(k.find_by_test_id("tall")[0])
            .unwrap()
            .frame
            .height,
        81.0,
        "the class's 80 and the separator"
    );
}

#[test]
fn a_symbol_under_a_condition_moves_the_text_with_it() {
    let mut r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"row\"\n      when dark\n        image \"symbol:person\"\n      text \"Profile\"");
    let x = |r: &Runner<NoData>| r.kernel().node(id(r, "row")).unwrap().frame.x;
    assert_eq!(x(&r), 32.0, "no symbol: 16 into the group");
    let row = id(&r, "row");
    r.dispatch(row, exact_runner::Event::Press).unwrap();
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    assert_eq!(x(&r), 72.0, "the symbol shown: the text after it");
    assert_eq!(
        r.kernel().grouped_list(id(&r, "list")).unwrap().sections[0].rows[0]
            .symbol
            .as_deref(),
        Some("person.crop.circle")
    );
}

#[test]
fn hidden_parts_and_a_row_of_texts_are_read_as_the_web_shows_them() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"pair\"\n      row\n        text \"A\"\n        text \"B\"\n    row display=\"none\"\n      text \"Hidden\"\n  section display=\"none\"\n    row\n      text \"Gone\"");
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    assert_eq!(list.sections.len(), 1, "a hidden section is none");
    assert_eq!(list.sections[0].rows.len(), 1, "a hidden row is none");
    assert!(
        list.sections[0].rows[0].custom,
        "two texts in a `row` are the row's own layout, not a subtitle"
    );
}

#[test]
fn a_plain_lists_sections_meet_and_a_label_is_one_text() {
    let r = boot("list appearance=\"auto\" listStyle=\"plain\" testId=\"list\" flex=1\n  section testId=\"a\"\n    button press=go\n      text \"A\"\n  section testId=\"b\"\n    button press=go\n      text \"B\"");
    let k = r.kernel();
    let (a, b) = (
        k.node(id(&r, "a")).unwrap().frame,
        k.node(id(&r, "b")).unwrap().frame,
    );
    assert_eq!(b.y, a.y + a.height, "no gap");
    assert!(refused(
        "list appearance=\"auto\"\n  section\n    footer\n      text \"a\"\n      text \"b\""
    )
    .contains("one `text`"));
}

#[test]
fn a_conditional_text_and_an_authors_column_are_styled_as_the_kernel_reads_them() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"row\"\n      when dark\n        text \"New\"\n      text \"Notifications\" testId=\"title\"\n    button press=go testId=\"card\"\n      row width=40 height=40\n      column testId=\"stack\"\n        text \"Maya\"\n        text \"+1 415\"");
    let k = r.kernel();
    let list = k.grouped_list(id(&r, "list")).unwrap();
    assert_eq!(
        list.sections[0].rows[0].title.as_deref(),
        Some("Notifications")
    );
    assert_eq!(
        k.node(id(&r, "title")).unwrap().style.flex_grow,
        1.0,
        "styled as the title it is while `dark` is false"
    );
    assert!(list.sections[0].rows[1].custom);
    assert!(
        k.node(id(&r, "stack")).unwrap().frame.height < 50.0,
        "two lines and no subtitle padding (30 more) in a custom row's column"
    );
}

#[test]
fn a_text_after_a_condition_is_styled_by_the_condition() {
    let mut r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"row\"\n      when dark\n        text \"New\"\n      text \"Notifications\" testId=\"title\"");
    let grow = |r: &Runner<NoData>| r.kernel().node(id(r, "title")).unwrap().style.flex_grow;
    assert_eq!(grow(&r), 1.0, "the title while `dark` is false");
    let row = id(&r, "row");
    r.dispatch(row, exact_runner::Event::Press).unwrap();
    assert_eq!(grow(&r), 0.0, "the value once `New` is the title");
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    assert_eq!(
        list.sections[0].rows[0].secondary.as_deref(),
        Some("Notifications")
    );
}

#[test]
fn a_subtitle_beside_a_conditional_checkmark_and_a_native_button_row() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"row\"\n      column testId=\"stack\"\n        text \"Dark\"\n        text \"Always\"\n      when dark\n        image \"symbol:checkmark\"\n    button press=go testId=\"plain\"\n      column testId=\"other\"\n        text \"A\"\n        text \"B\"\n      button press=go\n        text \"Go\"\n    button appearance=\"auto\" press=go testId=\"native\"\n      text \"Native\"");
    let k = r.kernel();
    assert!(
        k.node(id(&r, "stack")).unwrap().frame.height > 60.0,
        "subtitle padding: the kernel reads a subtitle cell"
    );
    assert!(
        k.node(id(&r, "other")).unwrap().frame.height < 50.0,
        "beside a text button the row is custom, and so is its column"
    );
    let list = k.grouped_list(id(&r, "list")).unwrap();
    assert!(list.sections[0].rows[0].subtitle);
    assert!(list.sections[0].rows[1].custom);
    assert!(
        list.sections[0].rows[2].custom,
        "a native button is carried, the platform's control"
    );
}

#[test]
fn a_subtitle_shown_by_a_condition_and_a_hidden_text() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"row\"\n      when dark\n        image \"symbol:notifications\"\n      column testId=\"stack\"\n        text \"Privacy\"\n        when dark\n          text \"Screen lock\"\n    button press=go\n      text \"Gone\" display=\"none\"\n      text \"Stay\" testId=\"stay\"");
    let k = r.kernel();
    assert!(
        k.node(id(&r, "stack")).unwrap().frame.height > 45.0,
        "subtitle padding though the second line is conditional"
    );
    let list = k.grouped_list(id(&r, "list")).unwrap();
    assert!(list.sections[0].rows[0].subtitle);
    assert_eq!(list.sections[0].rows[1].title.as_deref(), Some("Stay"));
    assert_eq!(
        k.node(id(&r, "stay")).unwrap().style.flex_grow,
        1.0,
        "a hidden text is not counted: `Stay` is the title"
    );
}

#[test]
fn round_three_shapes() {
    // A class that makes a row a native button.
    let src = app("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button class=Native press=go testId=\"native\"\n      text \"Native\"");
    let src = format!("style Native\n  appearance=\"auto\"\n{src}");
    contract::compile(&src)
        .unwrap_or_else(|e| panic!("a class-made native button row compiles: {e}"));
    // A conditional first subtitle line, a text field beside a column,
    // and a hidden leading symbol.
    let mut r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"row\"\n      column\n        when dark\n          text \"New\"\n        text \"Notifications\" testId=\"second\"\n    row testId=\"field\"\n      column testId=\"cols\"\n        text \"A\"\n        text \"B\"\n      input type=\"text\" value=\"x\"\n    button press=go testId=\"plain\"\n      image \"symbol:person\" display=\"none\"\n      text \"Title\"");
    let size = |r: &Runner<NoData>| r.kernel().node(id(r, "second")).unwrap().frame.height;
    let title = size(&r);
    assert!(
        r.kernel().node(id(&r, "cols")).unwrap().frame.height < 50.0,
        "beside a text field the column is the author's"
    );
    assert_eq!(
        r.kernel().node(id(&r, "plain")).unwrap().frame.x,
        32.0,
        "a hidden symbol reserves no inset"
    );
    let row = id(&r, "row");
    r.dispatch(row, exact_runner::Event::Press).unwrap();
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    assert!(
        size(&r) < title,
        "with `New` shown, `Notifications` is the 15-pt second line"
    );
}

#[test]
fn a_column_row_keeps_its_direction() {
    // LLP 1116 D2: a `column` straight in a section stacks its children, as
    // it does anywhere else (two bake-off apps saw one laid out as a strip):
    // two texts are the subtitle cell; a form of its own is the author's.
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    column testId=\"pair\"\n      text \"Title\" testId=\"t1\"\n      text \"Detail\" testId=\"t2\"\n    column testId=\"form\"\n      text \"How did you feel?\" testId=\"q\"\n      row testId=\"choices\"\n        button press=go\n          text \"A\"\n      text \"Okay\" testId=\"answer\"\n    column testId=\"one\"\n      text \"Alone\"");
    let k = r.kernel();
    let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
    let rows = &k.grouped_list(id(&r, "list")).unwrap().sections[0].rows;
    assert_eq!(
        (
            rows[0].title.as_deref(),
            rows[0].secondary.as_deref(),
            rows[0].subtitle,
            rows[0].custom
        ),
        (Some("Title"), Some("Detail"), true, false),
        "two texts in a column are the subtitle cell, as in a `button`'s column"
    );
    assert!(
        rows[1].custom,
        "a column of anything else is its own layout"
    );
    let (t1, t2) = (frame("t1"), frame("t2"));
    assert!(
        t2.y >= t1.y + t1.height,
        "the second line under the first: {t1:?} {t2:?}"
    );
    assert_eq!(t1.x, t2.x, "both at the text's margin");
    assert!(t2.height < t1.height, "the 15-pt second line");
    let (q, choices, answer) = (frame("q"), frame("choices"), frame("answer"));
    assert!(
        choices.y >= q.y + q.height && answer.y >= choices.y + choices.height,
        "stacked: {q:?} {choices:?} {answer:?}"
    );
    assert_eq!(
        frame("form").x,
        32.0,
        "16 into the card, as a row without an icon"
    );
    assert_eq!(
        frame("one").height,
        53.0,
        "one line: a cell's 52 and its separator"
    );
    let own = boot("list appearance=\"auto\" flex=1\n  section\n    column flex-direction=\"row\" testId=\"strip\"\n      text \"A\" testId=\"a\"\n      text \"B\" testId=\"b\"");
    let k = own.kernel();
    let (a, b) = (
        k.node(id(&own, "a")).unwrap().frame,
        k.node(id(&own, "b")).unwrap().frame,
    );
    assert_eq!(a.y, b.y, "the author's `flex-direction` stands");
}

#[test]
fn a_row_holding_its_own_stack_keeps_the_cells_margins() {
    // Two bake-off apps (team-kanban's cards, household-list's items): a
    // `row` around a `column` of a title, a second line and a row of
    // buttons had no space above or below, so its title touched the card
    // and its buttons the separator, on every host. A hand-built cell keeps
    // its content inside the content view's margins: 15 above and below, as
    // a `column` row has them.
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    row testId=\"card\"\n      column flex=1 min-width=0 gap=8 testId=\"stack\"\n        text \"Ada task one\" testId=\"title\"\n        text \"Ada\" testId=\"by\"\n        row gap=8 testId=\"actions\"\n          button press=go\n            text \"To Doing\"\n          button press=go\n            text \"Delete\"\n    row testId=\"item\"\n      input type=\"checkbox\" checked=on input=flip\n      when on\n        column flex=1 min-width=0 testId=\"lines\"\n          text \"Milk\"\n          text \"Added by Alice\"\n          text \"Checked by Bob\"\n      button press=go\n        text \"Delete\"\n    row testId=\"one\"\n      text \"No cards yet.\"\n    row testId=\"field\"\n      input value=\"\" placeholder=\"Card title\" flex=1\n      button press=go\n        text \"Add\"\n    row testId=\"pair\"\n      column\n        text \"Title\"\n        text \"Detail\"\n    row testId=\"cover\"\n      column width=44 height=66\n      text \"Untitled\"\n    button press=go testId=\"profile\"\n      row width=56 height=56 margin-top=10 margin-bottom=10\n      column\n        text \"Maya Chen\"\n        text \"+1 415 555 0134\"");
    let k = r.kernel();
    let frame = |t: &str| k.node(id(&r, t)).unwrap().frame;
    let rows = &k.grouped_list(id(&r, "list")).unwrap().sections[0].rows;
    assert!(rows[0].custom && rows[1].custom, "custom and carried");
    let (card, title, actions) = (frame("card"), frame("title"), frame("actions"));
    assert_eq!(title.y - card.y, 15.0, "the title 15 under the card's top");
    assert_eq!(
        card.y + card.height - 1.0 - (actions.y + actions.height),
        15.0,
        "the buttons 15 over the separator: {card:?} {actions:?}"
    );
    assert_eq!(title.x, 32.0, "16 into the card, as before");
    let (item, lines) = (frame("item"), frame("lines"));
    assert_eq!(
        lines.y - item.y,
        15.0,
        "a column beside controls, shown by a condition, too"
    );
    assert_eq!(
        item.height,
        lines.height + 31.0,
        "15 above, 15 below, and the separator"
    );
    assert_eq!(
        frame("one").height,
        53.0,
        "one text: a cell's 52 and its separator"
    );
    assert_eq!(
        frame("field").height,
        53.0,
        "a row of controls keeps the cell's 52"
    );
    assert!(
        rows[4].subtitle && frame("pair").height < 70.0,
        "the subtitle cell's padding is its column's, not its row's too"
    );
    assert_eq!(
        frame("cover").height,
        67.0,
        "an empty column is a box (a cover's placeholder), not lines"
    );
    assert_eq!(
        frame("profile").height,
        77.0,
        "the stack's margins are its own: a taller avatar with its own sets the height"
    );
    // The author's padding replaces the sheet's, a shorthand included.
    let own = boot("list appearance=\"auto\" flex=1\n  section\n    row padding-top=8 padding-bottom=8 testId=\"tight\"\n      column testId=\"in\"\n        text \"A\"\n        text \"B\"\n        text \"C\"\n    row padding=40 testId=\"loose\"\n      column testId=\"in2\"\n        text \"A\"\n        text \"B\"\n        text \"C\"");
    let k = own.kernel();
    let frame = |t: &str| k.node(id(&own, t)).unwrap().frame;
    assert!((frame("in").y - frame("tight").y - 8.0).abs() < 0.01);
    assert!((frame("in2").y - frame("loose").y - 40.0).abs() < 0.01);
}

#[test]
fn a_stack_of_lines_takes_the_subtitle_cells_text_styles() {
    // LLP 1116.000 D3.2: household-list's items (a title, who added it, who
    // checked it) drew three lines alike. A stack of lines in a row is drawn
    // as the subtitle cell draws its two: the first text the title, a later
    // one the subheadline in the secondary colour. What the author wrote wins,
    // row by row, on the text or on the stack its texts inherit from. The
    // sizes are text styles (LLP 1115.000 D3.3), read here at the kernel's
    // own root, CSS's 16: the body is 16 and the subheadline 14, where iOS's
    // 17 reads 17 and 15 (`the_sheets_text_follows_the_root_font_size`).
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go\n      column\n        text \"Privacy\" testId=\"cell-title\"\n        text \"Screen lock\" testId=\"cell-detail\"\n    row\n      input type=\"checkbox\" checked=on input=flip\n      column flex=1 min-width=0\n        text \"Milk\" testId=\"title\"\n        text \"Added by Alice\" testId=\"by\"\n        when on\n          text \"Checked by Bob\" testId=\"checked\"\n      button press=go\n        text \"Delete\" testId=\"delete\"\n    row\n      column flex=1 min-width=0 gap=8\n        text \"Ship it\" testId=\"card\"\n        text \"Ada\" font=\"-exact-footnote\" color=\"#123456\" testId=\"authored\"\n        text \"Late\" color=\"#ff0000\" testId=\"red\"\n        text \"Big\" font-size=20 testId=\"big\"\n        row gap=8\n          button press=go\n            text \"To Doing\" testId=\"move\"\n    row padding-top=8 padding-bottom=8\n      column color=\"#00ff00\" font-size=13\n        text \"A\" testId=\"own-a\"\n        text \"B\" testId=\"own-b\"\n        text \"C\" testId=\"own-c\"\n    row\n      column color=\"#00ff00\"\n        text \"Title\"\n        text \"Detail\" testId=\"cell-own\"");
    let k = r.kernel();
    let hex = |h: &str| Color::parse_hex(h).unwrap();
    let look = |t: &str| {
        let s = k.node(id(&r, t)).unwrap().computed_style(StyleMask::ALL);
        (
            s.font_size,
            s.text_color.resolve(false),
            s.text_color.resolve(true),
        )
    };
    let (title, detail) = (look("cell-title"), look("cell-detail"));
    assert_eq!(
        (title.0, detail.0),
        (16.0, 14.0),
        "the subtitle cell's own lines: the body and the subheadline"
    );
    assert_eq!(look("title"), title, "the first text is the title");
    assert_eq!(look("by"), detail, "a later one the subtitle's line");
    assert_eq!(look("checked"), detail, "one shown by a condition too");
    assert_eq!(look("card"), title, "beside a row of buttons");
    assert_ne!(look("delete"), detail, "a control's label is its own");
    assert_ne!(look("move"), detail, "and a button's in the stack");
    let authored = look("authored");
    assert!(authored.0 < 14.0, "an authored font wins: {authored:?}");
    assert_eq!(authored.1, hex("#123456"), "an authored colour wins");
    let red = look("red");
    assert_eq!((red.0, red.1), (14.0, hex("#ff0000")), "row by row");
    let big = look("big");
    assert_eq!((big.0, big.1, big.2), (20.0, detail.1, detail.2));
    // The stack's own rows reach its lines; a row with its own padding keeps
    // the styles (only its margins are the author's).
    for t in ["own-a", "own-b", "own-c"] {
        let own = look(t);
        assert_eq!((own.0, own.1), (13.0, hex("#00ff00")), "{t}");
    }
    let own = look("cell-own");
    assert_eq!(
        (own.0, own.1),
        (14.0, hex("#00ff00")),
        "the subtitle cell's too"
    );
}

/// LLP 1115.000 D3.3: the sheet's text sizes are text styles, so a grouped
/// list's text is the platform's body size and not iOS's 17 on every host: a
/// row and a header the body style, a second line the subheadline, a footer
/// the footnote, each read from the ramp at the root font size the host set.
/// The row's own metrics do not follow: a cell is 52 points at any of them.
#[test]
fn the_sheets_text_follows_the_root_font_size() {
    let body = "list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    header\n      text \"Account\" testId=\"header\"\n    row testId=\"row\"\n      text \"Title\" testId=\"title\"\n      text \"Value\" testId=\"value\"\n    button press=go testId=\"cell\"\n      column\n        text \"Privacy\" testId=\"line\"\n        text \"Screen lock\" testId=\"second\"\n    footer\n      text \"Who can see you.\" testId=\"footer\"";
    // (the root, then the body, the subheadline and the footnote there)
    for (root, name, sizes) in [
        (17.0, "iOS at its default size", (17.0, 15.0, 13.0)),
        (13.0, "the Mac", (13.0, 11.0, 10.0)),
        (16.0, "the web", (16.0, 14.0, 12.0)),
        (23.0, "iOS at its largest ordinary size", (23.0, 21.0, 19.0)),
    ] {
        let mut r = boot(body);
        // The kernel starts at CSS's 16; a host tells it its own.
        let told = r.set_root_font_size(root).unwrap();
        assert_eq!(told.is_some(), root != 16.0, "{name}");
        let k = r.kernel_mut();
        let page = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
        k.compute_layout(page, Offer::definite(402.0, 874.0))
            .unwrap();
        let k = r.kernel();
        let size = |t: &str| {
            let node = k.node(id(&r, t)).unwrap();
            node.computed_style(StyleMask::ALL).font_size
        };
        let (b, sub, foot) = sizes;
        for (t, want) in [
            ("header", b),
            ("title", b),
            ("value", b),
            ("line", b),
            ("second", sub),
            ("footer", foot),
        ] {
            assert_eq!(size(t), want, "{name}: {t}");
        }
        let row = k.node(id(&r, "row")).unwrap().frame.height;
        assert_eq!(row, 53.0, "{name}: a row is 52 and its separator");
    }
}

#[test]
fn a_hidden_text_in_a_subtitle_column_and_a_hidden_first_line() {
    let mut r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go\n      column testId=\"stack\"\n        text \"Gone\" display=\"none\"\n        text \"Privacy\"\n        text \"Screen lock\"\n    button press=go testId=\"row\"\n      column\n        when dark\n          text \"Privacy\"\n        text \"Screen lock\" testId=\"lock\"");
    assert!(
        r.kernel().node(id(&r, "stack")).unwrap().frame.height > 60.0,
        "two shown lines: the subtitle cell's padding"
    );
    let size = |r: &Runner<NoData>| r.kernel().node(id(r, "lock")).unwrap().frame.height;
    let alone = size(&r);
    let row = id(&r, "row");
    r.dispatch(row, exact_runner::Event::Press).unwrap();
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    assert!(
        size(&r) < alone,
        "the title alone, then the 15-pt second line under `Privacy`"
    );
}

#[test]
fn a_transparent_section_has_no_card() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section background-color=\"transparent\" testId=\"head\"\n    row testId=\"profile\"\n      image \"avatar.png\" width=80 height=80\n      text \"Maya Chen\"\n  section\n    button press=go testId=\"mute\"\n      text \"Mute\"");
    let k = r.kernel();
    let list = k.grouped_list(id(&r, "list")).unwrap();
    assert!(
        !list.sections[0].card,
        "background-color=\"transparent\" drops the card"
    );
    assert!(list.sections[1].card, "a section is on a card by default");
    // The sheet's group (the rows' column) takes the colour; the section
    // itself keeps none, so the web and the sheet's hosts draw no card either.
    let head = k.node(id(&r, "head")).unwrap();
    let group = k.node(head.children()[0]).unwrap();
    let transparent = group
        .style
        .background_color
        .is_some_and(|c| c.resolve(false).a() == 0 && c.resolve(true).a() == 0);
    assert!(transparent, "the group's background is the section's");
    assert!(
        head.style
            .background_color
            .is_none_or(|c| c.resolve(false).a() == 0),
        "and the section paints none of its own"
    );
}

#[test]
fn a_cardless_section_draws_no_separators_and_takes_only_transparent() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section background-color=\"transparent\"\n    button press=go testId=\"a\"\n      text \"A\"\n    button press=go testId=\"b\"\n      text \"B\"");
    let k = r.kernel();
    let a = k.node(id(&r, "a")).unwrap();
    assert_eq!(
        a.style.border_widths()[2],
        0.0,
        "no separator under a card-less row"
    );
    let m = refused(
        "list appearance=\"auto\" flex=1\n  section background-color=\"#ff0000\"\n    text \"A\"",
    );
    assert!(m.contains("transparent"), "{m}");
}

#[test]
fn an_authored_margin_is_the_webs_space_and_the_sheets_is_uikits() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"a\"\n      text \"Account\"\n  section\n    button press=go testId=\"b\"\n      text \"Chats\"\n  section margin-top=20 margin-bottom=0\n    button press=go testId=\"c\"\n      text \"Help\"");
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    let above: Vec<_> = list.sections.iter().map(|s| s.space_above).collect();
    assert_eq!(
        above,
        [None, None, Some(20.0)],
        "the sheet's numbers keep UIKit's gaps; the author's 20 is the space above"
    );
    assert_eq!(list.space_below, Some(0.0), "and the 0 under the last");
    // LLP 1115.000 D2: a margin is the author's because they wrote it, not
    // because its number differs from the sheet's. 17.33 between sections and
    // 0 beside a header are the sheet's own numbers, written here.
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go\n      text \"Account\"\n  section margin-top=17.33\n    button press=go\n      text \"Chats\"\n  section margin-top=0\n    header\n      text \"More\"\n    button press=go\n      text \"Help\"");
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    let above: Vec<_> = list.sections.iter().map(|s| s.space_above).collect();
    assert_eq!(
        above,
        [None, Some(17.33), Some(17.33)],
        "each written margin meets the sheet's 17.33 under the section before it"
    );
    assert_eq!(list.space_below, None, "the last one's bottom is unsaid");
}

#[test]
fn authored_margins_collapse_as_the_web_lays_them_out() {
    let body = "list appearance=\"auto\" testId=\"list\" flex=1\n  section margin-bottom=30\n    button press=go testId=\"a\"\n      text \"Account\"\n  section margin-top=40\n    header testId=\"more\"\n      text \"More\"\n    button press=go testId=\"b\"\n      text \"Chats\"\n  section margin-top=0.5\n    button press=go testId=\"c\"\n      text \"Help\"";
    let r = boot(body);
    let k = r.kernel();
    let list = k.grouped_list(id(&r, "list")).unwrap();
    assert_eq!(
        list.sections[0].space_above, None,
        "the first keeps UIKit's"
    );
    assert_eq!(
        list.sections[1].space_above,
        Some(40.0),
        "30 and 40 meet as 40, not 70, above the header"
    );
    // The kernel's own layout (which the web host follows) agrees: the
    // rows are 40 apart where the margins meet, less the separator's overlap.
    let a = k.node(id(&r, "a")).unwrap().frame;
    let header_top = k.node(id(&r, "more")).unwrap().frame.y;
    assert!(
        (header_top - (a.y + a.height - 1.0) - 40.0).abs() < 0.5,
        "web gap {}",
        header_top - (a.y + a.height - 1.0)
    );
    assert_eq!(
        list.sections[2].space_above,
        Some(17.33),
        "a half point is the author's: it meets the sheet's 17.33 under the last section"
    );
    assert_eq!(
        list.space_below, None,
        "the last section's bottom is the sheet's"
    );
}

#[test]
fn a_negative_margin_collapses_as_css_has_it() {
    let r = boot("list appearance=\"auto\" listStyle=\"plain\" testId=\"list\" flex=1\n  section margin-bottom=30\n    button press=go testId=\"a\"\n      text \"Account\"\n  section margin-top=-10\n    button press=go testId=\"b\"\n      text \"Chats\"\n  section margin-top=17.33\n    button press=go testId=\"c\"\n      text \"Help\"");
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    assert_eq!(
        list.sections[1].space_above,
        Some(20.0),
        "30 and -10 are 20 (CSS 2 §8.3.1), not 30"
    );
    assert_eq!(
        list.sections[2].space_above,
        Some(17.33),
        "17.33 is the author's: they wrote it"
    );
}

#[test]
fn default_buttons_keep_grouped_titles_presses_and_detail_accessories() {
    let body = "list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"open\"\n      text \"Open\"\n      image \"symbol:sf/chevron.right\"\n    button press=go testId=\"dark\"\n      text \"Dark\"\n      when dark\n        image \"symbol:checkmark\"\n    button press=go testId=\"delete\" destructive=true\n      text \"Delete Account\"\n    row testId=\"device\"\n      text \"Device\"\n      button press=go testId=\"info\" aria-label=\"Info\"\n        image \"symbol:sf/info.circle\"";
    let mut r = boot(body);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("grouped-default.contract");
    let (plan, map) = contract::compile_path_source_mapped(&path, &app(body)).unwrap();
    let json: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let reasons: Vec<_> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["bare_reason"].as_str())
        .collect();
    assert_eq!(
        reasons,
        [
            "grouped-list row",
            "grouped-list row",
            "grouped-list row",
            "grouped-list detail accessory"
        ]
    );
    assert!(
        map.button_migrations().is_empty(),
        "grouped-list rows need no migration"
    );
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    let rows = &list.sections[0].rows;
    for (index, title) in ["Open", "Dark", "Delete Account"].iter().enumerate() {
        assert_eq!(rows[index].title.as_deref(), Some(*title));
        assert!(rows[index].pressable, "{title} is a grouped-list action");
        assert!(!rows[index].custom);
        let node = r.kernel().node(rows[index].view).unwrap();
        assert_eq!(node.style.appearance, exact_kernel::Appearance::None);
    }
    assert_eq!(rows[0].accessory, Accessory::Disclosure);
    assert_eq!(rows[3].accessory, Accessory::Detail(id(&r, "info")));
    r.dispatch(id(&r, "dark"), exact_runner::Event::Press)
        .unwrap();
    assert_eq!(
        r.kernel().grouped_list(id(&r, "list")).unwrap().sections[0].rows[1].accessory,
        Accessory::Checkmark
    );
}

// @ref LLP 1115.002 D6 — the sheet names the platform's metrics and holds
// none of its numbers: a row reads its platform's column where it runs (the
// web's here, which is the sheet as it was). The names are the compiler's:
// an author's value that is one is refused, on a row that takes text too.
#[test]
fn the_sheet_names_the_platforms_metrics_and_an_author_cannot() {
    let body = "list appearance=\"auto\" testId=\"list\" flex=1\n  section testId=\"section\"\n    button press=go destructive=true testId=\"row\"\n      text \"Remove\"";
    let plan = contract::compile(&app(body)).unwrap();
    for name in [
        "-exact-grouped-ground",
        "-exact-grouped-card-fill",
        "-exact-grouped-card-radius",
        "-exact-grouped-button-min-height",
        "-exact-grouped-destructive-color",
        "-exact-grouped-button-appearance",
    ] {
        assert!(plan.strings.iter().any(|s| s == name), "{name}");
    }
    let r = boot(body);
    let row = r.kernel().node(id(&r, "row")).unwrap();
    assert_eq!(row.style.min_height, exact_kernel::Dimension::Points(52.0));
    assert_eq!(row.style.appearance, exact_kernel::Appearance::None);
    assert!(
        !row.style.mask.has(exact_kernel::StyleId::PaddingTop)
            && !row.style.mask.has(exact_kernel::StyleId::PaddingLeft),
        "the Mac's rows are no row here"
    );
    let red = ColorValue::Role(exact_kernel::style::roles::role("-exact-system-red").unwrap());
    assert_eq!(row.style.text_color, red);
    assert!(
        !row.style.written.has(exact_kernel::StyleId::TextColor),
        "the sheet's rows are the platform's, not the author's (LLP 1115.000 D2)"
    );
    for authored in [
        "column min-height=\"-exact-grouped-row-min-height\"",
        "text \"x\" color=\"-exact-grouped-destructive-color\"",
        "text \"x\" color=(dark ? \"-exact-grouped-destructive-color\" : \"#fff\")",
    ] {
        let e = contract::compile(&app(authored)).unwrap_err();
        assert_eq!(e.id, "lower-attr-value", "{authored}: {}", e.message);
    }
}

// @ref LLP 1116.007 D3.1 — the chevron by another spelling is still the disclosure.
#[test]
fn a_chevron_by_another_spelling_is_the_disclosure_accessory() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"open\"\n      text \"Open\"\n      image \"symbol:chevron-right\"");
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    assert_eq!(list.sections[0].rows[0].accessory, Accessory::Disclosure);
}

#[test]
fn a_standard_cells_text_keeps_its_line_clamp() {
    // LLP 1116.003 D7, round 5's chat-rooms: a room's last message, a
    // subtitle line written `line-clamp=1`, grew to 13 lines on iOS, where
    // UIKit draws the cell's text and wraps every line. The kernel reads how
    // many lines each text shows and whether a cut line ends in "…", as the
    // web lays it out; a text that says nothing keeps the platform's.
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"room\"\n      column flex=1 min-width=0\n        text \"General\"\n        text \"A long last message\" line-clamp=1\n      image \"symbol:forward-chevron\"\n    row\n      text \"Title\" line-clamp=2\n      text \"Value\" white-space=\"nowrap\" overflow=\"hidden\" text-overflow=\"ellipsis\"\n    row\n      text \"Kept\" white-space=\"nowrap\"\n      text \"Plain\"");
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    let rows = &list.sections[0].rows;
    let clamped = TextLook {
        lines: 1,
        ellipsis: true,
        ..TextLook::default()
    };
    assert_eq!(
        (rows[0].subtitle, rows[0].title_look, rows[0].secondary_look),
        (true, TextLook::default(), clamped),
        "the subtitle line is clamped, the title the platform's"
    );
    assert_eq!(rows[0].accessory, Accessory::Disclosure);
    assert_eq!(
        (rows[1].title_look, rows[1].secondary_look),
        (
            TextLook {
                lines: 2,
                ellipsis: true,
                ..TextLook::default()
            },
            clamped
        ),
        "a clamp of two, and a line kept whole with an ellipsis"
    );
    assert_eq!(
        (rows[2].title_look, rows[2].secondary_look),
        (
            TextLook {
                lines: 1,
                ellipsis: false,
                ..TextLook::default()
            },
            TextLook::default()
        ),
        "a line kept whole without one is cut at the edge"
    );
}

#[test]
fn an_authored_colour_and_font_reach_the_platforms_texts_and_unsaid_ones_stay_its() {
    // LLP 1116.003 D7, round 5's unit-converter: a validation message in a
    // section footer, written `color="-exact-system-red"`, was red on the web
    // and the footer's grey on iOS, where UIKit draws footers and standard
    // cells. Author > platform (LLP 1115): the kernel hands a host each
    // text's own colour and font, and nothing where the sheet's stand in for
    // the platform's, which keeps its own. "Own" is what the author wrote
    // (LLP 1115.000 D2), on the text or on a box it inherits from: the sheet's
    // own value, written, is the author's, and a value the compiler implied
    // (a heading's size) is not, whatever it is.
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    header\n      text \"Measure\" font=\"-exact-footnote\"\n    row\n      text \"Title\" color=\"#ff0000\"\n      text \"Value\" font-size=20\n    button press=go destructive=true\n      text \"Delete\"\n    footer\n      text \"Enter a number.\" color=\"-exact-system-red\"\n  section\n    header\n      text \"Plain\"\n    button press=go\n      column\n        text \"Privacy\"\n        text \"Screen lock\" font-style=\"italic\"\n    row\n      text \"Name\"\n      text \"Ada\"\n    footer color=\"#00ff00\"\n      text \"Green, on the footer.\"\n  section\n    row\n      text \"Unsaid\"\n    row\n      text \"Quiet\" color=\"-exact-secondary-label\"\n      text \"Loud\" color=\"-exact-label\"\n    row\n      text \"Same\" color=\"-exact-label\" font-size=17\n      text \"Too\" color=\"-exact-secondary-label\"\n    button press=go color=\"#00ff00\"\n      text \"On the row\"\n      text \"Value\"\n    row\n      text \"Heading\" role=\"heading\" aria-level=1\n    footer\n      text \"The platform's grey.\"");
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    let (s0, s1, s2) = (&list.sections[0], &list.sections[1], &list.sections[2]);
    let red = s0.footer_look.color.expect("the footer's own red");
    assert!(
        matches!(red, ColorValue::Role(_)),
        "a platform colour crosses as itself: {red:?}"
    );
    assert_eq!(s0.footer_look.font, None, "its font left unsaid");
    let header = s0.header_look.font.expect("the header's own font");
    assert!(header.size < 16.0 && header.weight == 400, "{header:?}");
    assert_eq!(s0.header_look.color, None);
    let rows = &s0.rows;
    assert_eq!(
        rows[0].title_look.color,
        Some(ColorValue::Fixed(Color::parse_hex("#ff0000").unwrap()))
    );
    assert_eq!(rows[0].title_look.font, None);
    assert_eq!(
        rows[0].secondary_look.color, None,
        "the value's colour is the sheet's"
    );
    assert_eq!(
        rows[0].secondary_look.font,
        Some(TextFont {
            size: 20.0,
            weight: 400,
            italic: false,
            family: 0,
            numeric: 0
        })
    );
    assert!(rows[1].destructive);
    assert_eq!(
        rows[1].title_look,
        TextLook::default(),
        "a destructive row's red is the sheet's"
    );
    assert_eq!(
        s1.header_look,
        TextLook::default(),
        "a header that says nothing"
    );
    let privacy = &s1.rows[0];
    assert!(privacy.subtitle);
    assert_eq!(privacy.title_look, TextLook::default());
    assert_eq!(
        privacy.secondary_look.font.map(|f| (f.size, f.italic)),
        Some((14.0, true)),
        "the subtitle line's size is the sheet's (the subheadline, at this root), its style the author's"
    );
    assert_eq!(privacy.secondary_look.color, None);
    assert_eq!(
        (s1.rows[1].title_look, s1.rows[1].secondary_look),
        (TextLook::default(), TextLook::default()),
        "a title and a value that say nothing"
    );
    assert_eq!(
        s1.footer_look.color,
        Some(ColorValue::Fixed(Color::parse_hex("#00ff00").unwrap())),
        "a colour on the footer reaches its text"
    );
    assert_eq!(
        (s2.rows[0].title_look, s2.footer_look),
        (TextLook::default(), TextLook::default())
    );
    // The sheet names a role for each text (LLP 1115 §3). A role its author
    // wrote is theirs: in another text's slot,
    let role = |name: &str| ColorValue::Role(exact_kernel::style::roles::role(name).unwrap());
    assert_eq!(
        (s2.rows[1].title_look.color, s2.rows[1].secondary_look.color),
        (
            Some(role("-exact-secondary-label")),
            Some(role("-exact-label"))
        )
    );
    // and in its own, where the value is the sheet's and the row says who
    // wrote it; so too the sheet's own size.
    let same = &s2.rows[2];
    assert_eq!(
        (same.title_look.color, same.secondary_look.color),
        (
            Some(role("-exact-label")),
            Some(role("-exact-secondary-label"))
        )
    );
    assert_eq!(
        same.title_look.font.map(|f| (f.size, f.weight)),
        Some((17.0, 400))
    );
    assert_eq!(same.secondary_look.font, None);
    // A colour written on the row reaches its title, as CSS inherits it,
    // and not its value, which the sheet colours itself.
    let green = Some(ColorValue::Fixed(Color::parse_hex("#00ff00").unwrap()));
    assert_eq!(
        (s2.rows[3].title_look.color, s2.rows[3].secondary_look.color),
        (green, None)
    );
    // A heading's size and weight are the compiler's: the cell's title
    // keeps the platform's font.
    assert_eq!(s2.rows[4].title.as_deref(), Some("Heading"));
    assert_eq!(s2.rows[4].title_look, TextLook::default());
}

/// A destructive row's title keeps the platform's red where it is the first
/// line only under a condition: the sheet cannot write its red there and
/// writes the label colour, which is still the sheet's and not the author's.
/// The row says so itself (LLP 1115.000 D2): no sheet row is written.
#[test]
fn a_destructive_rows_conditional_title_leaves_its_colour_unsaid() {
    let r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go destructive=true\n      column\n        when dark\n          text \"Sure?\"\n        text \"Delete\"");
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    let row = &list.sections[0].rows[0];
    assert!(row.destructive);
    assert_eq!(row.title.as_deref(), Some("Delete"));
    assert_eq!(
        row.title_look.color, None,
        "the sheet's label colour is unsaid"
    );
}

/// Round 6's event-rsvp (LLP 1116.004 D9): an action that can't be taken is
/// disabled, and a disabled row looks it everywhere the sheet draws, as
/// UIKit draws one: the tertiary label's grey in place of its red or its
/// label colour, bound or not.
#[test]
fn a_disabled_row_is_the_tertiary_labels_grey() {
    // Both are roles since the sheet named its colours (KRN-01; the red
    // with LLP 1115.000 D3.2, whose table holds iOS 27's measured pair).
    let role = |name: &str| ColorValue::Role(exact_kernel::style::roles::role(name).unwrap());
    let (grey, red) = (role("-exact-tertiary-label"), role("-exact-system-red"));
    assert_eq!(
        red.fallback(),
        ColorValue::LightDark(
            Color::rgba(0xff, 0x38, 0x3c, 255),
            Color::rgba(0xff, 0x42, 0x45, 255),
        ),
        "where no host has the platform's red, the one the sheet drew before"
    );
    let mut r = boot("list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go destructive=true disabled=true testId=\"off\"\n      text \"Delete\"\n    button press=go destructive=true disabled=dark testId=\"bound\"\n      text \"Delete\"\n    button press=go disabled=on testId=\"plain\"\n      text \"Open\"");
    let colour = |r: &Runner<NoData>, t: &str| r.kernel().node(id(r, t)).unwrap().style.text_color;
    assert_eq!(colour(&r, "off"), grey);
    assert_eq!(
        colour(&r, "bound"),
        red,
        "enabled, a destructive row is red"
    );
    assert_eq!(colour(&r, "plain"), grey);
    let bound = id(&r, "bound");
    r.dispatch(bound, exact_runner::Event::Press).unwrap();
    assert_eq!(
        colour(&r, "bound"),
        grey,
        "disabled once its condition holds"
    );
    // The grey, like the red, is the sheet's and so unsaid (LLP 1115.000 D2):
    // a host that draws the platform's list draws its own disabled row.
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    for row in &list.sections[0].rows {
        assert_eq!(row.title_look, TextLook::default(), "{:?}", row.title);
    }
}

/// LLP 1116.004 D2 (unit-converter, round 6): a text's line feed is what
/// its `white-space` makes it on the web: a space under `normal`, which
/// collapses it, and a line break under `pre-line`, which UIKit's footer and
/// cell then draw as two lines.
#[test]
fn a_line_feed_in_a_footer_or_title_follows_its_white_space() {
    let r = boot(
        "list appearance=\"auto\" testId=\"list\" flex=1
  section
    row
      text `Title\\nsecond` white-space=\"pre-line\"
    footer
      text `1 m = 3.28084 ft\\nResults rounded.`
  section
    row
      text \"Plain\"
    footer
      text `1 m = 3.28084 ft  \\n  Results rounded.` white-space=\"pre-line\"
",
    );
    let list = r.kernel().grouped_list(id(&r, "list")).unwrap();
    assert_eq!(
        list.sections[0].footer.as_deref(),
        Some("1 m = 3.28084 ft Results rounded."),
        "normal: one line, as the web draws it"
    );
    assert_eq!(
        list.sections[1].footer.as_deref(),
        Some("1 m = 3.28084 ft\nResults rounded."),
        "pre-line: two lines, the spaces around the break gone"
    );
    assert_eq!(
        list.sections[0].rows[0].title.as_deref(),
        Some("Title\nsecond")
    );
}

/// LLP 1116.004 D2 (polls, round 6): the sheet tints a row's leading symbol
/// and its checkmark with the accent, `AccentColor`, which follows the
/// nearest `accent-color` as a list cell's image and checkmark follow UIKit's
/// `tintColor`; it had written iOS 27's blue, so a teal app's checkmark was
/// blue. The chevron keeps its tertiary grey.
#[test]
fn the_sheets_symbol_and_checkmark_take_the_accent() {
    let r = boot(&SETTINGS.replace("    button press=go testId=\"dark\"\n      text \"Dark\"\n      when dark\n        image \"symbol:checkmark\"",
        "    button press=go testId=\"dark\"\n      text \"Dark\"\n      image \"symbol:checkmark\" testId=\"check\"\n    row\n      image \"symbol:sf/star\" -exact-tint-color=\"AccentColor\" testId=\"accent\"\n      text \"Accent\""));
    let k = r.kernel();
    let tint = |t: &str| {
        k.node(id(&r, t))
            .unwrap()
            .computed_style(StyleMask::ALL)
            .tint_color
    };
    let accent = tint("accent");
    assert!(accent.is_some());
    assert_eq!(tint("check"), accent, "the checkmark: the accent");
    let profile = k.node(id(&r, "profile")).unwrap();
    let symbol = k.node(profile.children()[0]).unwrap();
    assert_eq!(
        symbol.computed_style(StyleMask::ALL).tint_color,
        accent,
        "the leading symbol: the accent"
    );
    let chevron = k.node(*profile.children().last().unwrap()).unwrap();
    assert_ne!(
        chevron.computed_style(StyleMask::ALL).tint_color,
        accent,
        "the chevron: grey"
    );
}
