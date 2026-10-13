//! LLP 1115 §3, LLP 1115.000 D3.5: a link's unsaid colour is the platform's
//! link role (`LinkText`: the tint on iOS, `linkColor` on macOS, the accent
//! on the web). A run with an `href` in its paragraph is a link, and so is a
//! block `link` of text. The author's `color` wins; a `link` that holds a
//! row or a column, or has a `role`, is the author's row or button and keeps
//! inheriting, as the bare node it is.

use exact_kernel::style::roles::role;
use exact_kernel::{Color, ColorValue, Kernel, StyleId, StyleMask};
use exact_runner::{DataError, DataSource, Event, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(plan: exact_plan::Plan) -> Runner<NoData> {
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn node<'a>(r: &'a Runner<NoData>, id: &str) -> exact_kernel::NodeRef<'a> {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel().node_by_key(key).unwrap()
}

fn color(r: &Runner<NoData>, id: &str) -> ColorValue {
    node(r, id)
        .computed_style(StyleMask::of(StyleId::TextColor))
        .text_color
}

const PAGE: &str = "component App
  state on = true
  view
    column color=\"#336699\"
      text testId=\"p\"
        text \"Read \"
        text testId=\"a\" href=\"/about\" \"about\"
        text testId=\"b\" href=\"/b\" color=\"red\" \"red\"
        text testId=\"plain\" \" us\"
      link testId=\"block\" href=\"tel:+15550100\"
        text \"Call 555 0100\" testId=\"words\"
      link testId=\"mixed\" href=\"/x\"
        image \"symbol:share\"
        when on
          text \"Share\"
      link testId=\"authored\" href=\"/x\" color=\"red\"
        text \"Red\"
      link testId=\"own\" href=\"/x\"
        text \"Green\" color=\"green\" testId=\"own-words\"
      link testId=\"row\" href=\"/x\"
        row
          text \"A row that navigates\" testId=\"row-words\"
      link testId=\"tab\" href=\"/x\" role=\"tab\"
        text \"A tab\"
      link testId=\"empty\" href=\"/x\" width=44 height=44
      list appearance=\"auto\" flex=1
        section
          link testId=\"cell\" href=\"/x\"
            text \"A row of a grouped list\"
            image \"symbol:forward-chevron\"
";

#[test]
fn a_link_is_link_text_unless_its_author_colours_it() {
    let r = boot(contract::compile(PAGE).unwrap_or_else(|e| panic!("{e}")));
    let link = ColorValue::Role(role("LinkText").unwrap());
    let page = ColorValue::Fixed(Color::rgba(0x33, 0x66, 0x99, 255));
    let red = ColorValue::Fixed(Color::rgba(255, 0, 0, 255));
    // A run with an `href` in its paragraph.
    assert_eq!(color(&r, "a"), link);
    assert_eq!(color(&r, "b"), red);
    assert_eq!(color(&r, "plain"), page, "a run without an href inherits");
    // A block `link` of text: the role is the link's, and its text's by
    // inheritance; the compiler's row, so no host reads it as written.
    assert_eq!(color(&r, "block"), link);
    assert_eq!(color(&r, "words"), link, "its text inherits it");
    let block = node(&r, "block");
    assert!(block.style.mask.has(StyleId::TextColor), "a row of its own");
    assert!(
        !block.style.written.has(StyleId::TextColor),
        "implied, not written (LLP 1115.000 D2)"
    );
    assert_eq!(
        color(&r, "mixed"),
        link,
        "text and images, a `when` of them"
    );
    assert_eq!(color(&r, "authored"), red, "an authored colour wins");
    assert!(node(&r, "authored").style.written.has(StyleId::TextColor));
    assert_eq!(
        color(&r, "own-words"),
        ColorValue::Fixed(Color::rgba(0, 128, 0, 255)),
        "and one on its text"
    );
    // The author's row or button keeps inheriting.
    assert_eq!(color(&r, "row"), page, "a link holding a row is a row");
    assert_eq!(color(&r, "row-words"), page);
    assert_eq!(color(&r, "tab"), page, "a link given a role is that");
    assert_eq!(color(&r, "empty"), page, "nothing to colour");
    assert_eq!(
        color(&r, "cell"),
        ColorValue::Role(role("-exact-label").unwrap()),
        "a grouped list's row is the sheet's label colour, a link or not"
    );
}

/// An `href` that turns out empty is no link: the implied colour follows the
/// value, as the kernel's `Props::link` does for every host. A package that
/// writes `href=r.href` on every run of a paragraph (`@exact/reading`'s
/// `Runs`) had all its prose link-blue; a run's colour now comes and goes
/// with its target, and so does a block link's.
#[test]
fn an_href_that_turns_out_empty_is_not_a_link() {
    let src = "component App
  state to = \"\"
  action go
    to = to == \"\" ? \"/about\" : \"\"
  view
    column color=\"#336699\"
      button press=go testId=\"go\"
        text \"Go\"
      text testId=\"p\"
        text testId=\"run\" href=to \"about\"
        text testId=\"none\" href=\"\" \" us\"
        text testId=\"red\" href=to color=\"red\" \" red\"
        text testId=\"made\" href=`/people/${to}` \" made\"
      link testId=\"block\" href=to
        text \"Call\" testId=\"words\"
      link testId=\"blank\" href=\"\"
        text \"Blank\"
";
    let mut r = boot(contract::compile(src).unwrap_or_else(|e| panic!("{e}")));
    let link = ColorValue::Role(role("LinkText").unwrap());
    let page = ColorValue::Fixed(Color::rgba(0x33, 0x66, 0x99, 255));
    let red = ColorValue::Fixed(Color::rgba(255, 0, 0, 255));
    let target = |r: &Runner<NoData>, id: &str| node(r, id).props.link().map(str::to_owned);
    // Empty: the paragraph's colour, and no target for a host to read.
    for id in ["run", "none", "block", "words", "blank"] {
        assert_eq!(color(&r, id), page, "{id} is not a link");
        assert_eq!(target(&r, id), None, "{id}");
    }
    assert!(
        !node(&r, "run").style.mask.has(StyleId::TextColor),
        "the row is unset, not a colour of its own"
    );
    assert_eq!(color(&r, "red"), red, "an authored colour wins");
    assert_eq!(
        color(&r, "made"),
        link,
        "a template's own words are a target"
    );
    // The `href` row is still there: the node is HTML's `<a>` with no
    // `href`, laid out as one.
    assert_eq!(
        node(&r, "block").props.str(exact_kernel::PropId::Href),
        Some("")
    );
    // A target arrives: the link's colour, with no author's row.
    let go = node(&r, "go").id;
    r.dispatch(go, Event::Press).unwrap();
    for id in ["run", "block", "words"] {
        assert_eq!(color(&r, id), link, "{id} is a link");
    }
    assert_eq!(target(&r, "run").as_deref(), Some("/about"));
    assert!(!node(&r, "run").style.written.has(StyleId::TextColor));
    assert_eq!(color(&r, "none"), page);
    assert_eq!(color(&r, "red"), red);
    // And leaves again.
    r.dispatch(go, Event::Press).unwrap();
    for id in ["run", "block", "words"] {
        assert_eq!(color(&r, id), page, "{id} is no link again");
    }
    assert_eq!(color(&r, "red"), red);
}

/// A terminal entry's block link is as it was: what a terminal shows for a
/// link is the terminal's own question, not yet decided (`QUEUE.md`), so
/// LLP 1115.000 D3.5 changes nothing there. Its link run is the role, as
/// before.
#[test]
fn a_terminal_entrys_block_link_keeps_inheriting() {
    let src = "component App
  view
    column color=\"#336699\"
      text testId=\"p\"
        text \"Read \"
        text testId=\"a\" href=\"/about\" \"about\"
      link testId=\"block\" href=\"/x\"
        text \"Block\"
";
    let r = boot(contract::compile_terminal(src).unwrap_or_else(|e| panic!("{e}")));
    let page = ColorValue::Fixed(Color::rgba(0x33, 0x66, 0x99, 255));
    assert_eq!(color(&r, "block"), page);
    assert_eq!(color(&r, "a"), ColorValue::Role(role("LinkText").unwrap()));
}
