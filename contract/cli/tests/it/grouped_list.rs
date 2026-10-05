//! LLP 1084: `list appearance="auto"` is a grouped list. Contract checks its
//! sections and writes its look as a sheet the author's rows replace; the
//! kernel reads its sections and rows as a native list draws them.

use exact_kernel::{Accessory, GroupedRow, Kernel, Offer, PropId};
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
    assert_eq!(frame("notes").y - profile.y, 52.0);
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
