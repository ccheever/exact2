//! @ref LLP 1095 D4 — `theme`: an app's own colours named once at its
//! root, an entry named like a role overriding it; the plan carries the
//! table, and a colour string naming an entry reads its colour, in the
//! compiler and when the runner sets the row.

use exact_kernel::{Color, ColorValue, Kernel, RowValue, StyleId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn colour(r: &Runner<NoData>, id: &str, row: StyleId) -> ColorValue {
    let key = r.kernel().find_by_test_id(id)[0];
    match r.kernel().node_by_key(key).unwrap().computed(row) {
        RowValue::ColorValue(c) => c,
        RowValue::Color(c) => ColorValue::Fixed(c),
        other => panic!("{id}: {other:?}"),
    }
}

fn parsed(text: &str) -> ColorValue {
    ColorValue::parse_light_dark(text)
        .or_else(|| Color::parse(text).map(ColorValue::Fixed))
        .unwrap_or_else(|| panic!("{text}"))
}

const THEME: &str = "theme\n  brand = \"#c8102e\"\n  card = \"secondary-grouped-background\"\n  defrost-on = \"light-dark(#d0e2f7, #27394f)\"\n  secondary-label = \"-exact-platform-color(ios secondaryLabelColor, light-dark(#6b6b73, #a1a1aa))\"\n  ink = \"brand\"\n  -exact-tertiary-label = \"defrost-on\"\n  Canvas = \"#fafafa\"\n\n";

#[test]
fn names_read_their_entry_and_a_role_entry_overrides_the_role() {
    let src = format!("{THEME}component App\n  state picked: string = \"brand\"\n  action pick(v: string)\n    picked = v\n  view\n    column testId=\"card\" background-color=\"card\"\n      text \"a\" testId=\"brand\" color=\"brand\"\n      text \"b\" testId=\"pair\" color=\"defrost-on\"\n      text \"c\" testId=\"ink\" color=\"ink\"\n      text \"d\" testId=\"override\" color=\"-exact-secondary-label\"\n      text \"e\" testId=\"chained\" color=\"-exact-tertiary-label\"\n      text \"f\" testId=\"system\" color=\"canvas\"\n      text \"g\" testId=\"computed\" color=picked\n");
    let mut r = boot(&src);
    let fixed = parsed("#c8102e");
    assert_eq!(colour(&r, "brand", StyleId::TextColor), fixed);
    assert_eq!(colour(&r, "ink", StyleId::TextColor), fixed);
    assert_eq!(
        colour(&r, "pair", StyleId::TextColor),
        parsed("light-dark(#d0e2f7, #27394f)")
    );
    assert_eq!(
        colour(&r, "card", StyleId::BackgroundColor),
        parsed("-exact-secondary-grouped-background")
    );
    assert_eq!(
        colour(&r, "override", StyleId::TextColor),
        parsed("-exact-platform-color(ios secondaryLabelColor, light-dark(#6b6b73, #a1a1aa))")
    );
    assert_eq!(
        colour(&r, "chained", StyleId::TextColor),
        parsed("light-dark(#d0e2f7, #27394f)")
    );
    assert_eq!(
        colour(&r, "system", StyleId::TextColor),
        parsed("#fafafa"),
        "a CSS role, any case"
    );
    // A string from state names an entry when it runs.
    assert_eq!(colour(&r, "computed", StyleId::TextColor), fixed);
    r.act("pick", vec![Value::str("defrost-on")]).unwrap();
    assert_eq!(
        colour(&r, "computed", StyleId::TextColor),
        parsed("light-dark(#d0e2f7, #27394f)")
    );
    r.act("pick", vec![Value::str("nope")]).unwrap();
    assert_eq!(
        colour(&r, "computed", StyleId::TextColor),
        parsed("CanvasText"),
        "an unknown name is an invalid colour: the row's initial"
    );
    // The plan carries the table.
    let plan = contract::compile(&src).unwrap();
    let rows: Vec<(&str, &str, &str)> = plan
        .theme
        .iter()
        .map(|t| (plan.str(t.name), plan.str(t.role), plan.str(t.value)))
        .collect();
    assert!(rows.contains(&("ink", "", "#c8102e")), "{rows:?}");
    assert!(
        rows.contains(&(
            "secondary-label",
            "-exact-secondary-label",
            "-exact-platform-color(ios secondaryLabelColor, light-dark(#6b6b73, #a1a1aa))"
        )),
        "{rows:?}"
    );
    assert!(
        rows.contains(&("card", "", "-exact-secondary-grouped-background")),
        "{rows:?}"
    );
}

#[test]
fn a_theme_name_is_a_colour_in_every_colour_row_and_embedded_colour() {
    let rows = [
        ("text \"a\"", "color=\"brand\""),
        ("text \"a\"", "background-color=\"brand\""),
        ("text \"a\"", "border-color=\"brand\""),
        ("text \"a\"", "border-top-color=\"brand\""),
        ("text \"a\"", "-exact-tint-color=\"brand\""),
        ("text \"a\"", "-webkit-text-stroke-color=\"brand\""),
        ("text \"a\"", "box-shadow=\"0 1px 2px brand\""),
        ("text \"a\"", "text-shadow=\"0 1px 2px brand\""),
        (
            "text \"a\"",
            "background-image=\"linear-gradient(brand, defrost-on)\"",
        ),
        (
            "text \"a\"",
            "mask-image=\"linear-gradient(brand, transparent)\"",
        ),
        ("text \"a\"", "filter=\"drop-shadow(0 1px 2px brand)\""),
        ("input value=\"\" aria-label=\"x\"", "caret-color=\"brand\""),
        (
            "input value=\"\" aria-label=\"x\"",
            "accent-color=\"brand\"",
        ),
        ("view column-count=\"2\"", "column-rule-color=\"brand\""),
        ("view", "-exact-symbol-palette=\"brand ink\""),
    ];
    // Each reads what the literal it names would.
    let style = |src: &str| {
        let r = boot(src);
        let k = r.kernel();
        let key = k.find_by_test_id("it")[0];
        format!("{:?}", k.node_by_key(key).unwrap().style)
    };
    for (element, row) in rows {
        let src = format!("{THEME}component App\n  view\n    {element} testId=\"it\" {row}\n");
        contract::compile(&src).unwrap_or_else(|e| panic!("{row}: {e:?}"));
        let literal = row
            .replace("defrost-on", "light-dark(#d0e2f7, #27394f)")
            .replace("brand", "#c8102e")
            .replace("ink", "#c8102e");
        let plain = format!("component App\n  view\n    {element} testId=\"it\" {literal}\n");
        assert_eq!(style(&src), style(&plain), "{row}");
    }
    let svg = format!("{THEME}component App\n  view\n    svg viewBox=\"0 0 10 10\" width=\"10px\" height=\"10px\"\n      rect width=\"10\" height=\"10\" fill=\"brand\" stroke=\"ink\"\n");
    contract::compile(&svg).unwrap_or_else(|e| panic!("svg: {e:?}"));
}

#[test]
fn a_theme_is_checked_where_it_is_declared() {
    for (body, id) in [
        ("  a = \"b\"\n  b = \"a\"\n", "lower-theme-cycle"),
        (
            "  secondary-label = \"-exact-secondary-label\"\n",
            "lower-theme-cycle",
        ),
        (
            "  label = \"-exact-platform-color(ios labelColor, label)\"\n",
            "lower-theme-value",
        ),
        (
            "  brand = \"light-dark(ink, #000)\"\n  ink = \"#000\"\n",
            "lower-theme-value",
        ),
        ("  brand = \"blurple\"\n", "lower-theme-value"),
        ("  red = \"#c8102e\"\n", "lower-theme-name"),
        (
            "  brand = \"#000\"\n  brand = \"#111\"\n",
            "syntax-duplicate-declaration",
        ),
        ("  brand = 3\n", "syntax-theme-entry"),
    ] {
        let src = format!("theme\n{body}\ncomponent App\n  view\n    text \"a\"\n");
        let errors = contract::compile(&src).unwrap_err();
        assert!(format!("{errors:?}").contains(id), "{body}: {errors:?}");
    }
    // A row naming no entry, role or colour is refused as it was.
    let src = format!("{THEME}component App\n  view\n    text \"a\" color=\"brnad\"\n");
    assert!(format!("{:?}", contract::compile(&src).unwrap_err()).contains("lower-attr-value"));
}

/// A role the theme replaces is replaced where the kernel resolves it: its
/// own defaults (unsaid text is `CanvasText`) and what crosses to a host,
/// not only a row naming it. A plan without the override puts it back.
#[test]
fn an_overridden_role_is_replaced_where_the_kernel_resolves_it() {
    let src = "theme\n  CanvasText = \"light-dark(#123456, #abcdef)\"\n  separator = \"-exact-system-red\"\n\ncomponent App\n  view\n    column\n      text \"a\" testId=\"unsaid\"\n      view testId=\"rule\" border-top=\"1px solid -exact-separator\"\n";
    let r = boot(src);
    let unsaid = colour(&r, "unsaid", StyleId::TextColor);
    assert_eq!(unsaid, parsed("CanvasText"), "the row is the role");
    assert_eq!(unsaid.resolve(false), Color::parse("#123456").unwrap());
    assert_eq!(unsaid.resolve(true), Color::parse("#abcdef").unwrap());
    assert_eq!(unsaid.fallback(), parsed("light-dark(#123456, #abcdef)"));
    let mut wire = String::new();
    exact_kernel::gradient::color_text(&mut wire, unsaid, exact_kernel::gradient::ColorText::Wire);
    assert_eq!(wire, "light-dark(#123456ff, #abcdefff)");
    // An override naming another role crosses as that role.
    assert_eq!(
        colour(&r, "rule", StyleId::BorderColorTop),
        parsed("-exact-system-red")
    );
    drop(r);
    let plain = boot("component App\n  view\n    text \"a\" testId=\"unsaid\"\n");
    let mut wire = String::new();
    exact_kernel::gradient::color_text(
        &mut wire,
        colour(&plain, "unsaid", StyleId::TextColor),
        exact_kernel::gradient::ColorText::Wire,
    );
    assert_eq!(wire, "CanvasText");
}
