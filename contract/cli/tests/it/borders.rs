//! LLP 1053 G2: per-side border colours by their CSS names — the four
//! longhands, `border-color` with one to four values, and `currentcolor`
//! kept a keyword that follows `color`, never an RGB baked at compile time.

use exact_kernel::{Color, ColorValue, Kernel, StyleId, StyleMask, StyleProps};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn style_of(r: &Runner<NoData>, id: &str) -> StyleProps {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0])
        .unwrap()
        .style
        .clone()
}

/// The four sides as painted: `currentcolor` resolved against the node's
/// computed `color`, as every host resolves it.
fn painted(r: &Runner<NoData>, id: &str) -> [ColorValue; 4] {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let current = node
        .computed_style(StyleMask::of(StyleId::TextColor))
        .text_color;
    node.style.border_colors(current)
}

fn fixed(hex: &str) -> ColorValue {
    ColorValue::Fixed(Color::parse_hex(hex).unwrap())
}

fn sides(s: &StyleProps) -> [Option<ColorValue>; 4] {
    [
        s.border_color_top,
        s.border_color_right,
        s.border_color_bottom,
        s.border_color_left,
    ]
}

#[test]
fn each_longhand_sets_its_side_and_an_unset_side_stays_currentcolor() {
    let r = boot(
        "component A\n  view\n    column testId=\"box\" color=\"#112233\" border-width=2 border-style=\"solid\" border-top-color=\"#ff0000\" border-right-color=\"#00ff00\" border-left-color=\"light-dark(#0000ff, #ffff00)\"\n",
    );
    let s = style_of(&r, "box");
    let pair = ColorValue::LightDark(
        Color::parse_hex("#0000ff").unwrap(),
        Color::parse_hex("#ffff00").unwrap(),
    );
    assert_eq!(
        sides(&s),
        [
            Some(fixed("#ff0000")),
            Some(fixed("#00ff00")),
            None,
            Some(pair)
        ]
    );
    assert_eq!(
        painted(&r, "box"),
        [fixed("#ff0000"), fixed("#00ff00"), fixed("#112233"), pair]
    );
}

#[test]
fn border_color_takes_one_to_four_values_as_css_expands_them() {
    let (a, b, c, d) = ("#aa0000", "#00bb00", "#0000cc", "#dddd00");
    let r = boot(&format!(
        "component A\n  view\n    column\n      box testId=\"one\" border-color=\"{a}\"\n      box testId=\"two\" border-color=\"{a} {b}\"\n      box testId=\"three\" border-color=\"{a}  {b}\t{c}\"\n      box testId=\"four\" border-color=\"{a} {b} {c} {d}\"\n      box testId=\"pairs\" border-color=\"light-dark(#000000, #ffffff) currentcolor\"\n"
    ));
    let f = |h| Some(fixed(h));
    assert_eq!(sides(&style_of(&r, "one")), [f(a), f(a), f(a), f(a)]);
    assert_eq!(sides(&style_of(&r, "two")), [f(a), f(b), f(a), f(b)]);
    assert_eq!(sides(&style_of(&r, "three")), [f(a), f(b), f(c), f(b)]);
    assert_eq!(sides(&style_of(&r, "four")), [f(a), f(b), f(c), f(d)]);
    let pair = Some(ColorValue::LightDark(
        Color::parse_hex("#000000").unwrap(),
        Color::parse_hex("#ffffff").unwrap(),
    ));
    assert_eq!(
        sides(&style_of(&r, "pairs")),
        [pair, None, pair, None],
        "a `light-dark()` is one value; `currentcolor` stays the keyword"
    );
}

#[test]
fn a_later_declaration_wins_whether_shorthand_or_longhand_and_a_class_yields() {
    let r = boot(
        "style Framed\n  border-color=\"#111111 #222222\"\n  border-left-color=\"#333333\"\ncomponent A\n  view\n    column\n      box testId=\"long-last\" border-color=\"#aaaaaa\" border-top-color=\"#bbbbbb\"\n      box testId=\"short-last\" border-top-color=\"#bbbbbb\" border-color=\"#aaaaaa\"\n      box testId=\"classed\" class=Framed border-right-color=\"#444444\"\n",
    );
    let f = |h| Some(fixed(h));
    assert_eq!(
        sides(&style_of(&r, "long-last")),
        [f("#bbbbbb"), f("#aaaaaa"), f("#aaaaaa"), f("#aaaaaa")]
    );
    assert_eq!(sides(&style_of(&r, "short-last")), [f("#aaaaaa"); 4]);
    assert_eq!(
        sides(&style_of(&r, "classed")),
        [f("#111111"), f("#444444"), f("#111111"), f("#333333")]
    );
}

#[test]
fn a_computed_border_color_splits_each_arm_and_currentcolor_follows_color() {
    let r = boot(
        "component A\n  state on = false\n  action flip\n    on = not on\n  view\n    column\n      button testId=\"flip\" press=flip width=10 height=10\n      box testId=\"box\" color=(on ? \"#00ff00\" : \"#ff0000\") border-color=(on ? \"#000000 currentcolor\" : \"currentcolor\") border-bottom-color=\"currentcolor\"\n",
    );
    let red = fixed("#ff0000");
    let green = fixed("#00ff00");
    assert_eq!(painted(&r, "box"), [red; 4]);
    let mut r = r;
    let flip = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id
    };
    r.dispatch(flip, Event::Press).unwrap();
    assert_eq!(
        painted(&r, "box"),
        [fixed("#000000"), green, green, green],
        "the unresolved sides follow the new `color`; an explicit `currentcolor` too"
    );
}

#[test]
fn a_bad_side_or_a_fifth_value_is_refused_at_compile_time() {
    for (value, says) in [
        (
            "#ff0000 #00ff00 #0000ff #ffffff #000000",
            "one to four colours",
        ),
        ("#ff0000 blurple", "not a valid `border-color`"),
    ] {
        let src = format!("component A\n  view\n    box border-color=\"{value}\"\n");
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, "lower-attr-value", "{value}: {e}");
        assert!(e.message.contains(says), "{value}: {e}");
    }
    let e =
        contract::compile("component A\n  view\n    box border-left-color=\"#12\"\n").unwrap_err();
    assert_eq!(e.id, "lower-attr-value", "{e}");
}
