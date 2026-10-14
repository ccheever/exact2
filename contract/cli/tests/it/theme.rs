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
    // The plan carries the table, each value as the crate folds it (LLP
    // 1121 D7: CSS's serialization).
    let plan = contract::compile(&src).unwrap();
    let rows: Vec<(&str, &str, &str)> = plan
        .theme
        .iter()
        .map(|t| (plan.str(t.name), plan.str(t.role), plan.str(t.value)))
        .collect();
    assert!(rows.contains(&("ink", "", "#c8102eff")), "{rows:?}");
    assert!(
        rows.contains(&(
            "secondary-label",
            "-exact-secondary-label",
            "-exact-platform-color(ios secondaryLabelColor, light-dark(#6b6b73ff, #a1a1aaff))"
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
        ("  a = \"b\"\n  b = \"a\"\n", "tamber-cycle"),
        (
            "  secondary-label = \"-exact-secondary-label\"\n",
            "tamber-cycle",
        ),
        (
            "  label = \"-exact-platform-color(ios labelColor, label)\"\n",
            "tamber-cycle",
        ),
        ("  brand = \"blurple\"\n", "tamber-unknown-token"),
        ("  red = \"#c8102e\"\n", "tamber-name-reserved"),
        (
            "  brand = \"#000\"\n  brand = \"#111\"\n",
            "syntax-duplicate-declaration",
        ),
        ("  brand = 3\n", "syntax-theme-entry"),
        (
            "  secondary-label = \"#000\"\n  -exact-secondary-label = \"#111\"\n",
            "tamber-duplicate",
        ),
        ("  LinkText = \"#000\"\n", "lower-theme-tint"),
        ("  link = \"#000\"\n", "lower-theme-tint"),
        ("  none = \"#000\"\n", "tamber-name-reserved"),
        ("  inherit = \"#000\"\n", "tamber-name-reserved"),
        ("  top = \"#000\"\n", "lower-theme-name"),
        ("  srgb = \"#000\"\n", "lower-theme-name"),
        ("  AccentColor = \"LinkText\"\n", "lower-theme-tint"),
    ] {
        let src = format!("theme\n{body}\ncomponent App\n  view\n    text \"a\"\n");
        let errors = contract::compile(&src).unwrap_err();
        assert!(format!("{errors:?}").contains(id), "{body}: {errors:?}");
    }
    // A row naming no token, role or colour (LLP 1121 D3).
    let src = format!("{THEME}component App\n  view\n    text \"a\" color=\"brnad\"\n");
    assert!(format!("{:?}", contract::compile(&src).unwrap_err()).contains("tamber-unknown-token"));
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

/// Each plan's names are its own, bound per parse: two runners on one
/// thread, each with its own `brand`, each read their own as their rows
/// are set; and the second's role overrides taking the thread's over is
/// said in its journal. An unknown name is an invalid colour in the log.
#[test]
fn two_plans_each_read_their_own_names() {
    let app = |brand: &str, canvas: &str| {
        format!("theme\n  brand = \"{brand}\"\n  CanvasText = \"{canvas}\"\n\ncomponent App\n  state c: string = \"brand\"\n  action pick(v: string)\n    c = v\n  view\n    text \"a\" testId=\"t\" color=c\n")
    };
    // Compiled, not baked: a bake boots the plan once itself.
    let run = |src: &str| {
        Runner::boot(
            contract::compile(src).unwrap(),
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    };
    let mut red = run(&app("#ff0000", "#010101"));
    let mut blue = run(&app("#0000ff", "#020202"));
    assert!(
        blue.journal().any(|l| l.contains("replace another plan's")),
        "{:?}",
        blue.journal().collect::<Vec<_>>()
    );
    for _ in 0..2 {
        red.act("pick", vec![Value::str("x")]).unwrap();
        red.act("pick", vec![Value::str("brand")]).unwrap();
        blue.act("pick", vec![Value::str("x")]).unwrap();
        blue.act("pick", vec![Value::str("brand")]).unwrap();
        assert_eq!(colour(&red, "t", StyleId::TextColor), parsed("#ff0000"));
        assert_eq!(colour(&blue, "t", StyleId::TextColor), parsed("#0000ff"));
    }
    let before = red.journal().count();
    red.act("pick", vec![Value::str("nope")]).unwrap();
    let said: Vec<&str> = red.journal().skip(before).collect();
    assert!(said.iter().any(|l| l.contains("nope")), "{said:?}");
}

/// A name is a name, exactly: `Brand` and `brand` are two entries (only a
/// role's spelling matches without case); an entry may be a declared
/// profile's colour; a theme's one-scheme colour is the lint's as a literal
/// is, named by a row or replacing a role the page paints.
#[test]
fn names_are_exact_profiles_are_colours_and_the_lint_reads_the_theme() {
    let src = "theme\n  Brand = \"#ff0000\"\n  brand = \"#0000ff\"\n\ncomponent App\n  view\n    column\n      text \"a\" testId=\"lower\" color=\"brand\"\n      text \"b\" testId=\"upper\" color=\"Brand\"\n";
    let r = boot(src);
    assert_eq!(colour(&r, "lower", StyleId::TextColor), parsed("#0000ff"));
    assert_eq!(colour(&r, "upper", StyleId::TextColor), parsed("#ff0000"));
    let icc = "color-profile --brand src=\"assets/brand.icc\"\n\ntheme\n  ink = \"color(--brand 0.1 0.2 0.3 0.4)\"\n\ncomponent App\n  view\n    text \"a\" color=\"ink\"\n";
    contract::compile(icc).unwrap_or_else(|e| panic!("{e:?}"));
    let lint = |src: &str| {
        let (result, warnings) = contract::compile_warned(src);
        result.unwrap();
        warnings
            .iter()
            .filter(|w| w.id == "lower-colour-one-scheme")
            .count()
    };
    assert!(lint("theme\n  card = \"#f1f3f5\"\n\ncomponent App\n  view\n    column background-color=\"card\"\n      text \"a\"\n") > 0, "a row naming a one-scheme entry");
    assert!(
        lint("theme\n  Canvas = \"#fbf3e9\"\n\ncomponent App\n  view\n    text \"a\"\n") > 0,
        "a page role replaced in one scheme"
    );
    assert!(lint("theme\n  base = \"#ffffff\"\n  card = \"base\"\n\ncomponent App\n  view\n    column background-color=\"card\"\n      text \"a\"\n") > 0, "through an alias");
    assert_eq!(lint("theme\n  card = \"light-dark(#f1f3f5, #0f1419)\"\n  Canvas = \"light-dark(#fff, #000)\"\n\ncomponent App\n  view\n    column background-color=\"card\"\n      text \"a\"\n"), 0);
}

/// A plan with no overrides booted after one with them clears them, and
/// says so: a session still running the first now shows none.
#[test]
fn clearing_another_plans_overrides_is_said() {
    let run = |src: &str| {
        Runner::boot(
            contract::compile(src).unwrap(),
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    };
    let _themed =
        run("theme\n  CanvasText = \"#010101\"\n\ncomponent App\n  view\n    text \"a\"\n");
    let plain = run("component App\n  view\n    text \"a\"\n");
    assert!(plain
        .journal()
        .any(|l| l.contains("replace another plan's")));
}

/// A boot the runner refuses puts back the overrides it took: a session
/// still running keeps its own.
#[test]
fn a_refused_boot_keeps_the_threads_overrides() {
    let run = |src: &str| {
        Runner::boot(
            contract::compile(src).unwrap(),
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
    };
    let kept = run(
        "theme\n  CanvasText = \"#010101\"\n\ncomponent App\n  view\n    text \"a\" testId=\"t\"\n",
    )
    .unwrap();
    let refused = run("theme\n  CanvasText = \"#020202\"\n\ncomponent App\n  state n = 0\n  state bad = 1 / n\n  view\n    text \"a\"\n");
    assert!(refused.is_err(), "the fixture is refused at boot");
    assert_eq!(
        colour(&kept, "t", StyleId::TextColor).resolve(false),
        Color::parse("#010101").unwrap()
    );
}

/// A role replaced by a tint role is the tint: under the theme's
/// `AccentColor`, unsaid text that names `LinkText` paints the accent.
#[test]
fn a_role_replaced_by_a_tint_role_paints_the_tint() {
    let r = boot("theme\n  CanvasText = \"LinkText\"\n  AccentColor = \"#ff0000\"\n\ncomponent App\n  view\n    text \"a\" testId=\"t\"\n");
    assert_eq!(
        colour(&r, "t", StyleId::TextColor).resolve(false),
        Color::parse("#ff0000").unwrap()
    );
}
