//! LLP 1082: `list appearance="auto"` is a grouped list. Contract checks its
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
