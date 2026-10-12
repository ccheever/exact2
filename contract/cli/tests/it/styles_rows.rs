//! CSS rows beside the named styles (styles.rs): `order`, the CSS-wide
//! keywords and `currentcolor`, and `position-area`, proven on the kernel's
//! rows after boot. Split from styles.rs.

use exact_kernel::{Color, Kernel};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// CSS `order` is a box's style row, bound or literal: a flex item moves in
/// its row, as on the web (feed F19: it compiled as an SVG filter
/// primitive's attribute and moved nothing).
#[test]
fn order_moves_a_flex_item_and_a_bound_order_moves_it_again() {
    let src = "component A\n  state rail = true\n  action flip\n    rail = not rail\n  view\n    row testId=\"row\" width=300\n      view testId=\"main\" width=200 height=10\n      view testId=\"rail\" width=100 height=10 order=(rail ? -1 : 0)\n      view testId=\"last\" width=0 height=10 order=1\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let x = |r: &mut Runner<NoData>, id: &str| {
        let root = r.roots()[0];
        r.kernel_mut()
            .compute_layout(root, exact_kernel::Offer::definite(400.0, 400.0))
            .unwrap();
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0]).unwrap().frame.x
    };
    assert_eq!((x(&mut r, "rail"), x(&mut r, "main")), (0.0, 100.0));
    r.act("flip", vec![]).unwrap();
    assert_eq!((x(&mut r, "main"), x(&mut r, "rail")), (0.0, 200.0));
    assert_eq!(x(&mut r, "last"), 300.0);
}

/// CSS-wide keywords and `currentcolor` (feed F1): `inherit` on an
/// inherited row and `unset` leave the row unset, so the parent's colour
/// shows, bound or literal, over a class's row; `currentcolor` on a
/// background or a tint is the keyword, which each host paints in `color`.
#[test]
fn inherit_unset_and_currentcolor_are_csss() {
    let src = "style Loud\n  color=\"#ff0000\"\ncomponent A\n  state on = false\n  action flip\n    on = not on\n  view\n    column color=\"#0000ff\"\n      text \"a\" testId=\"inherit\" class=Loud color=\"inherit\"\n      text \"b\" testId=\"bound\" color=(on ? \"unset\" : \"#00ff00\")\n      view testId=\"swatch\" background-color=\"currentcolor\"\n      image \"symbol:add\" testId=\"icon\" -exact-tint-color=\"currentColor\" alt=\"Add\"\n      input testId=\"field\" value=\"\" enterkeyhint=\"send\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let node = |r: &Runner<NoData>, id: &str| {
        let k = r.kernel();
        let key = k.find_by_test_id(id)[0];
        let n = k.node_by_key(key).unwrap();
        (n.style.clone(), n.text_color())
    };
    let blue: exact_kernel::ColorValue = Color::parse_hex("#0000ff").unwrap().into();
    let (inherit, color) = node(&r, "inherit");
    assert!(!inherit.mask.has(exact_kernel::StyleId::TextColor));
    assert_eq!(color, blue, "the class's red is not the node's");
    assert_eq!(
        node(&r, "bound").1,
        Color::parse_hex("#00ff00").unwrap().into()
    );
    r.act("flip", vec![]).unwrap();
    assert_eq!(node(&r, "bound").1, blue);
    assert!(
        r.journal().all(|l| !l.contains("invalid")),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
    let (swatch, _) = node(&r, "swatch");
    assert!(swatch.mask.has(exact_kernel::StyleId::BackgroundColor));
    assert_eq!(swatch.background_color, None);
    let (icon, _) = node(&r, "icon");
    assert_eq!(icon.tint_color, None);
    let k = r.kernel();
    let icon = k.node_by_key(k.find_by_test_id("icon")[0]).unwrap();
    assert_eq!(
        icon.props.str(exact_kernel::PropId::AccessibilityLabel),
        Some("Add")
    );
    let field = k.node_by_key(k.find_by_test_id("field")[0]).unwrap();
    assert_eq!(
        field.props.str(exact_kernel::PropId::EnterKeyHint),
        Some("send")
    );
}

#[test]
fn position_area_places_a_popover_in_the_admitted_subset_only() {
    let popover = |value: &str| {
        format!("component App\n  view\n    column\n      button \"Open\" popovertarget=\"p\"\n      column id=\"p\" popover=\"auto\" position-area=\"{value}\"\n        text \"x\"\n")
    };
    for value in [
        "none",
        "bottom",
        "bottom span-right",
        "bottom span-all",
        "top",
        "top span-right",
        "top span-all",
        "center",
        "right span-bottom",
    ] {
        contract::compile(&popover(value)).unwrap_or_else(|e| panic!("{value}: {e}"));
    }
    for value in [
        "left",
        "right",
        "right span-top",
        "span-bottom right",
        "top left",
        "span-all top",
        "top span-left",
        "block-start",
        "nonsense",
    ] {
        let error = contract::compile(&popover(value)).unwrap_err().to_string();
        assert!(
            error.contains("subset of CSS `position-area`") && error.contains("top span-all"),
            "{value}: {error}"
        );
    }
    let error = contract::compile("component App\n  view\n    column position-area=\"top\"\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("on a `popover` only"), "{error}");
}

/// LLP 1115.000 D2: "written" is a fact each row carries, from where the
/// compiler emits it to the node the kernel keeps. An attribute and a
/// class's row are the author's; a tag's fixed row, a heading's style, a
/// link run's colour, `hr`'s rows and a sheet's are not, and where two set
/// one property the node has the winner's.
#[test]
fn a_node_knows_which_of_its_rows_its_author_wrote() {
    use exact_kernel::StyleId::{
        Appearance, BackgroundColor, Display, FontSize, FontWeight, MarginBottom, MarginTop,
        MinHeight, PaddingTop, TextColor, TintColor, Width,
    };
    let src = r##"style Loud
  color="#ff0000"
  padding=4
style Quiet
  color="#888888"
component A
  state on = false
  action flip
    on = not on
  view
    column testId="page" color="#111111"
      text "own" testId="own" color="-exact-system-red"
      text "same" testId="same" color="#111111"
      text "inherits" testId="inherits"
      text "class" testId="class" class=Loud
      text "both" testId="both" class=Loud color="#00ff00"
      text "choice" testId="choice" class=(on ? Loud : Quiet)
      text "Heading" testId="heading" role="heading" aria-level=1
      text "Sized" testId="sized" role="heading" font-size=20
      text testId="para"
        text "See "
        text "the docs" testId="link" href="https://example.com"
        text "in red" testId="link-red" href="https://example.com" color="#ff0000"
      hr testId="rule"
      hr testId="rule-red" color="#ff0000"
      textarea testId="editor" value="hi" markup="markdown"
      textarea testId="editor-none" value="hi" markup="markdown" appearance="none"
      row testId="row"
      view testId="bound" width=(on ? 10 : 20)
      list appearance="auto" testId="list"
        section testId="section" margin-bottom=30
          header testId="header"
            text "Head"
          button press=flip testId="said" color="-exact-label"
            text "Says the sheet's own colour"
          button press=flip testId="unsaid"
            text "Says nothing"
          button press=flip disabled=true testId="off"
            image "symbol:sf/star" testId="icon"
            text "Off"
            image "symbol:checkmark" testId="check"
          row
            image "symbol:sf/star" -exact-tint-color="AccentColor" testId="tinted"
            text "Tinted"
"##;
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // (set, written) for a row of the node `id`.
    let row = |r: &Runner<NoData>, id: &str, row: exact_kernel::StyleId| {
        let k = r.kernel();
        let style = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().style;
        (style.mask.has(row), style.written.has(row))
    };
    let (written, implied, unset) = ((true, true), (true, false), (false, false));
    for (id, style, expected, why) in [
        ("page", TextColor, written, "an attribute"),
        ("own", TextColor, written, "a platform colour is a colour"),
        ("same", TextColor, written, "equal to its parent's"),
        ("inherits", TextColor, unset, "inherited: no row, no bit"),
        ("class", TextColor, written, "a class's row"),
        ("class", PaddingTop, written, "a class's shorthand"),
        ("both", TextColor, written, "its own over its class's"),
        ("choice", TextColor, written, "a row both chosen styles set"),
        ("choice", PaddingTop, unset, "the chosen style sets none"),
        ("heading", FontSize, implied, "a heading's size"),
        ("heading", FontWeight, implied, "and its weight"),
        ("sized", FontSize, written, "the author's size wins"),
        ("sized", FontWeight, implied, "beside the heading's weight"),
        ("link", TextColor, implied, "a link run's colour"),
        ("link-red", TextColor, written, "its author's colour"),
        ("rule", TextColor, implied, "`hr`'s separator colour"),
        ("rule", MarginTop, implied, "and its margins"),
        ("rule-red", TextColor, written, "the author's on an `hr`"),
        ("rule-red", MarginTop, implied, "keeps the tag's other rows"),
        ("row", Display, implied, "a tag's fixed row"),
        ("editor", Appearance, implied, "a field never native"),
        ("editor-none", Appearance, written, "the same, written"),
        ("bound", Width, written, "a computed attribute"),
        ("list", BackgroundColor, implied, "the grouped sheet's"),
        ("section", MarginBottom, written, "over the sheet's"),
        ("section", MarginTop, implied, "the sheet's"),
        ("header", TextColor, implied, "the sheet's label colour"),
        ("said", TextColor, written, "the sheet's value, written"),
        ("said", MinHeight, implied, "beside the sheet's metrics"),
        ("unsaid", TextColor, implied, "the sheet's"),
        ("off", TextColor, implied, "the sheet's disabled grey"),
        ("icon", TintColor, implied, "the sheet's accent on a symbol"),
        ("check", TintColor, implied, "and on a checkmark"),
        ("tinted", TintColor, written, "the same accent, written"),
    ] {
        assert_eq!(row(&r, id, style), expected, "{id} {style:?}: {why}");
    }
    // The other choice sets the row: its bit comes with it.
    r.act("flip", vec![]).unwrap();
    assert_eq!(row(&r, "choice", PaddingTop), written);
    assert_eq!(row(&r, "bound", Width), written);
}
