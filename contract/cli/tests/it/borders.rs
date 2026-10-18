//! LLP 1053 G2: per-side border colours by their CSS names — the four
//! longhands, `border-color` with one to four values, and `currentcolor`
//! kept a keyword that follows `color`, never an RGB baked at compile time.

use exact_kernel::{BorderStyle, Color, ColorValue, Kernel, StyleId, StyleMask, StyleProps};
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

#[test]
fn patterned_border_styles_lower_for_sides_shorthand_and_binding() {
    let mut r = boot(
        "component A\n  state dashed = false\n  action flip\n    dashed = not dashed\n  view\n    column\n      button testId=\"flip\" press=flip width=10 height=10\n      box testId=\"sides\" border-width=\"1px 2px 4px 3px\" border-style=\"dotted dashed solid none\"\n      box testId=\"long\" border-bottom-width=1 border-bottom-style=\"dotted\"\n      box testId=\"short\" border-bottom=\"4px dashed red\"\n      box testId=\"bound\" border-width=2 border-style=(dashed ? \"dashed\" : \"dotted\")\n",
    );
    let sides = style_of(&r, "sides");
    assert_eq!(
        [
            sides.border_style_top,
            sides.border_style_right,
            sides.border_style_bottom,
            sides.border_style_left
        ],
        [
            BorderStyle::Dotted,
            BorderStyle::Dashed,
            BorderStyle::Solid,
            BorderStyle::None
        ]
    );
    assert_eq!(sides.border_widths(), [1.0, 2.0, 4.0, 0.0]);
    assert_eq!(
        style_of(&r, "long").border_style_bottom,
        BorderStyle::Dotted
    );
    assert_eq!(
        style_of(&r, "short").border_style_bottom,
        BorderStyle::Dashed
    );
    assert_eq!(style_of(&r, "bound").border_style_top, BorderStyle::Dotted);
    let flip = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id
    };
    r.dispatch(flip, Event::Press).unwrap();
    assert_eq!(style_of(&r, "bound").border_style_top, BorderStyle::Dashed);
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
            "one to four values",
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

#[test]
fn padding_margin_border_width_and_inset_take_one_to_four_values_as_css_expands_them() {
    // Each shorthand against its longhands: `"a b c"` is top a, right b, bottom c, left b.
    let r = boot(
        "component A\n  view\n    column\n      box testId=\"short\" padding=\"1px 2px 3px\" margin=\"4px 5%\" border-width=\"6px 7px 8px 9px\" border-style=\"solid none\" position=\"absolute\" inset=\"calc(50% - 4px) 10px\"\n      box testId=\"long\" padding-top=1 padding-right=2 padding-bottom=3 padding-left=2 margin-top=4 margin-right=\"5%\" margin-bottom=4 margin-left=\"5%\" border-top-width=6 border-right-width=7 border-bottom-width=8 border-left-width=9 border-top-style=\"solid\" border-right-style=\"none\" border-bottom-style=\"solid\" border-left-style=\"none\" position=\"absolute\" top=\"calc(50% - 4px)\" right=10 bottom=\"calc(50% - 4px)\" left=10\n      box testId=\"one\" padding=\"12px\" padding-left=3\n",
    );
    let (short, long) = (style_of(&r, "short"), style_of(&r, "long"));
    macro_rules! same {
        ($($field:ident),*) => { $(assert_eq!(short.$field, long.$field, stringify!($field));)* };
    }
    same!(
        padding_top,
        padding_right,
        padding_bottom,
        padding_left,
        margin_top,
        margin_right,
        margin_bottom,
        margin_left,
        border_width_top,
        border_width_right,
        border_width_bottom,
        border_width_left,
        border_style_top,
        border_style_right,
        border_style_bottom,
        border_style_left,
        top,
        right,
        bottom,
        left
    );
    let one = style_of(&r, "one");
    assert_eq!(one.padding_top, one.padding_bottom);
    assert_ne!(
        one.padding_top, one.padding_left,
        "a later longhand still wins"
    );
    let e = contract::compile("component A\n  view\n    box padding=\"1px 2px 3px 4px 5px\"\n")
        .unwrap_err();
    assert!(
        format!("{e}").contains("`padding` takes one to four values"),
        "{e}"
    );
}

/// Ledger2 Rough 5: `border-radius` is CSS's one-to-four-value corner
/// shorthand (top-left, top-right, bottom-right, bottom-left), in an
/// attribute and a `style`, as `padding` is a side's.
#[test]
fn border_radius_takes_one_to_four_corners() {
    let r = boot(
        "style Sheet\n  border-radius=\"18px 9px\"\ncomponent A\n  view\n    column\n      box border-radius=\"18px 18px 0 0\" testId=\"top\" width=40 height=40\n      box border-radius=\"1px 2px 3px\" testId=\"three\" width=40 height=40\n      box class=Sheet testId=\"sheet\" width=40 height=40\n      box border-radius=\"50%\" testId=\"one\" width=40 height=40\n",
    );
    let corners = |id: &str| {
        let s = style_of(&r, id);
        [
            s.border_radius_top_left,
            s.border_radius_top_right,
            s.border_radius_bottom_right,
            s.border_radius_bottom_left,
        ]
        .map(|c| format!("{c:?}"))
    };
    let [tl, tr, br, bl] = corners("top");
    assert!(tl == tr && br == bl && tl != br, "{:?}", corners("top"));
    let [tl, tr, br, bl] = corners("three");
    assert!(
        tr == bl && tl != tr && br != tr && tl != br,
        "{:?}",
        corners("three")
    );
    let [tl, tr, br, bl] = corners("sheet");
    assert!(tl == br && tr == bl && tl != tr, "{:?}", corners("sheet"));
    let [tl, tr, br, bl] = corners("one");
    assert!(tl == tr && tr == br && br == bl, "{:?}", corners("one"));
    let e = contract::compile(
        "component A\n  view\n    box border-radius=\"18px / 9px 1px 2px 3px\"\n",
    )
    .unwrap_err();
    assert!(
        format!("{e}").contains("is an elliptical radius (`/`), which no host draws"),
        "{e}"
    );
}

#[test]
fn a_numeric_border_says_to_quote_the_shorthand() {
    let says = |attr: &str| {
        contract::compile(&format!(
            "component App\n  state x = true\n  view\n    view {attr} testId=\"b\"\n"
        ))
        .unwrap_err()
        .message
    };
    // What the message says to write compiles, and earns no warning: its
    // colour is a role, not a literal `lower-colour-one-scheme` would name.
    let compiles = |attr: &str| {
        let (plan, warnings) = contract::compile_warned(&format!(
            "component App\n  state x = true\n  view\n    view {attr} testId=\"b\"\n"
        ));
        plan.unwrap_or_else(|e| panic!("{attr}: {e}"));
        assert_eq!(warnings, [], "{attr}");
    };
    assert_eq!(
        says("border=0"),
        "`border`: `0` is a number, and a CSS shorthand is a string, so write `\"0\"`"
    );
    compiles("border=\"0\"");
    assert_eq!(says("border-top=2"), "`border-top`: `2` is a number, and a CSS shorthand is a string, as CSS writes it: `\"2px solid -exact-separator\"`");
    compiles("border-top=\"2px solid -exact-separator\"");
    // One arm of a choice: the literal is named, and its replacement keeps the choice.
    assert_eq!(says("border=(x ? \"0\" : 1)"), "`border`: `1` is a number, and a CSS shorthand is a string, as CSS writes it: `\"1px solid -exact-separator\"`");
    compiles("border=(x ? \"0\" : \"1px solid -exact-separator\")");
    // A negative width (a style folds `-1` into a number) gets no example: no width is negative.
    let error = contract::compile(
        "style Card\n  border=-1\ncomponent App\n  view\n    view class=Card testId=\"b\"\n",
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "`border`: `-1` is a number, and a CSS shorthand is a string, as CSS writes it"
    );
    // text-decoration has no number form to suggest.
    let error =
        contract::compile("component App\n  view\n    text \"a\" text-decoration=1\n").unwrap_err();
    assert_eq!(
        error.message,
        "`text-decoration`: `1` is a number, and a CSS shorthand is a string, as CSS writes it"
    );
}

/// The euxy port: a computed value that builds a list for a box shorthand
/// is refused, naming the longhands, since no host splits one at run time;
/// a computed single value is every side's, and a literal list is split.
#[test]
fn a_computed_shorthand_list_is_refused_naming_the_longhands() {
    for (attr, says) in [
        ("padding=`${pad}px 0`", "`padding` is computed here as a list of values, which no host splits at run time; write `padding-top`/`padding-right`/`padding-bottom`/`padding-left` (each may be computed), or a literal `padding`"),
        ("border-radius=radii(8)", "write `border-top-left-radius`/`border-top-right-radius`/`border-bottom-right-radius`/`border-bottom-left-radius`"),
        ("border-width=(on ? `${pad}px 0` : \"0\")", "write `border-top-width`/"),
        ("inset=\"0 \" + `${pad}px`", "write `top`/`right`/`bottom`/`left`"),
        ("margin=`${pad}px ${pad}px`", "write `margin-top`/"),
    ] {
        let e = contract::compile(&format!(
            "fn radii(r: number): string = `${{r}}px ${{r}}px 0 0`\ncomponent A\n  state pad = 16\n  state on = false\n  view\n    box {attr}\n"
        ))
        .unwrap_err();
        assert_eq!(e.id, "lower-css-shorthand", "{attr}: {e}");
        assert!(e.message.contains(says), "{attr}: {e}");
    }
    // One computed value, a calc, a literal list or a literal arm: compiled.
    let r = boot(
        "component A\n  state pad = 16\n  state on = false\n  view\n    column\n      box testId=\"one\" padding=`${pad}px`\n      box testId=\"calc\" padding=`calc(${pad}px + 2px)`\n      box testId=\"arm\" padding=(on ? \"1px\" : \"16px 0\")\n      box testId=\"num\" padding=pad\n",
    );
    let (one, arm) = (style_of(&r, "one"), style_of(&r, "arm"));
    assert_eq!(
        format!("{:?}", one.padding_left),
        format!("{:?}", one.padding_top)
    );
    assert_eq!(
        format!("{:?}", arm.padding_top),
        format!("{:?}", one.padding_top)
    );
    assert_eq!(arm.padding_left, exact_kernel::Dimension::Points(0.0));
    assert!(style_of(&r, "calc").mask.has(StyleId::PaddingLeft));
    assert_eq!(
        style_of(&r, "num").padding_left,
        exact_kernel::Dimension::Points(16.0)
    );
}

/// An empty value declares nothing, as CSSOM's `setProperty(p, "")`: the
/// row is unset with no journal line, and a component's prop of `""` under
/// `if dot != ""` compiles (the euxy port passed `"transparent"` instead).
#[test]
fn an_empty_colour_declares_nothing() {
    let mut r = boot(
        "component A\n  state c = \"#ff0000\"\n  action clear\n    c = \"\"\n  view\n    column\n      button testId=\"clear\" press=clear width=10 height=10\n      box testId=\"bound\" background-color=c border-color=c\n      Dot(dot=\"\")\n      Dot(dot=\"#00ff00\")\n\ncomponent Dot\n  props\n    dot: string\n  view\n    row\n      if dot != \"\"\n        box testId=\"dot\" background-color=dot\n",
    );
    assert_eq!(
        style_of(&r, "bound").border_color_top,
        Some(fixed("#ff0000"))
    );
    let clear = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("clear")[0]).unwrap().id
    };
    r.dispatch(clear, Event::Press).unwrap();
    let bound = style_of(&r, "bound");
    assert!(!bound.mask.has(StyleId::BackgroundColor));
    assert_eq!(sides(&bound), [None; 4]);
    assert!(
        !r.journal().any(|l| l.contains("invalid")),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
    assert!(contract::compile("component A\n  view\n    box background-color=\"\"\n").is_ok());
}

/// A bound `0` is a length, as CSS's unitless zero is (`border-width` is a
/// number row). A list that reaches a box shorthand's rows, which the
/// compiler cannot see build, is refused by every row: all four unset
/// with a journal line, never split.
#[test]
fn a_bound_zero_is_a_length_and_a_bound_list_is_refused_by_every_row() {
    let r = boot(
        "component A\n  state z = \"0\"\n  state p = \"10px -5px\"\n  state round = \"10px / 20px\"\n  view\n    column\n      box testId=\"w\" border-style=\"solid\" border-width=2 border-left-width=z\n      box testId=\"p\" padding=p\n      box testId=\"r\" border-radius=round\n",
    );
    let w = style_of(&r, "w");
    assert_eq!(w.border_widths(), [2.0, 2.0, 2.0, 0.0]);
    let p = style_of(&r, "p");
    for row in [
        StyleId::PaddingTop,
        StyleId::PaddingRight,
        StyleId::PaddingBottom,
        StyleId::PaddingLeft,
    ] {
        assert!(!p.mask.has(row), "{row:?} is unset with the rest");
    }
    let radius = style_of(&r, "r");
    for row in exact_kernel::style::sides::FAMILIES[8] {
        assert!(!radius.mask.has(row), "{row:?} is unset with the rest");
    }
    let journal: Vec<_> = r.journal().collect();
    assert!(
        journal.iter().any(|l| l.contains("10px -5px"))
            && journal.iter().any(|l| l.contains("10px / 20px")),
        "{journal:?}"
    );
}

/// An empty `font-family`, literal, a choice's arm or a component's prop,
/// declares nothing, as an empty colour does.
#[test]
fn an_empty_font_family_declares_nothing() {
    let mut r = boot(
        "component A\n  state plain = false\n  action flip\n    plain = not plain\n  view\n    column\n      button testId=\"flip\" press=flip width=10 height=10\n      text \"a\" testId=\"empty\" font-family=\"\"\n      text \"b\" testId=\"choice\" font-family=(plain ? \"\" : \"Georgia\")\n      Fam(f=\"\")\n\ncomponent Fam\n  props\n    f: string\n  view\n    text \"c\" testId=\"prop\" font-family=f\n",
    );
    assert!(!style_of(&r, "empty").mask.has(StyleId::FontFamily));
    assert!(!style_of(&r, "prop").mask.has(StyleId::FontFamily));
    assert!(style_of(&r, "choice").mask.has(StyleId::FontFamily));
    let flip = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id
    };
    r.dispatch(flip, Event::Press).unwrap();
    assert!(!style_of(&r, "choice").mask.has(StyleId::FontFamily));
}

/// A longhand bound to a list is not split: two lengths on a corner are
/// the elliptical radius no row holds, unset with a journal line. A keyword
/// is ASCII case-insensitive (`0 Auto` centres, as CSS's does); a literal
/// `/` is the elliptical refusal, whatever its count.
#[test]
fn a_longhand_list_is_not_split_and_keywords_ignore_case() {
    let r = boot(
        "component A\n  state c = \"10px 20px\"\n  view\n    column\n      box testId=\"corner\" border-top-left-radius=c\n      box testId=\"auto\" margin=\"0 Auto\" width=10 height=10\n",
    );
    assert!(!style_of(&r, "corner")
        .mask
        .has(StyleId::BorderRadiusTopLeft));
    let journal: Vec<_> = r.journal().collect();
    assert!(
        journal.iter().any(|l| l.contains("10px 20px")),
        "{journal:?}"
    );
    let auto = style_of(&r, "auto");
    assert_eq!(auto.margin_left, exact_kernel::Dimension::Auto);
    assert_eq!(auto.margin_right, exact_kernel::Dimension::Auto);
    assert_eq!(auto.margin_top, exact_kernel::Dimension::Points(0.0));
    for literal in ["10px / 20px", "10px/20px"] {
        let e = contract::compile(&format!(
            "component A\n  view\n    box border-radius=\"{literal}\"\n"
        ))
        .unwrap_err();
        assert!(
            e.message.contains("elliptical radius (`/`)"),
            "{literal}: {e}"
        );
    }
}

/// The euxy port (FRICTION 3): a choice of literal corner lists, on a swipe
/// row in a component whose props pick the arm, is split into the four
/// corners as each arm's literal would be.
#[test]
fn a_choice_of_corner_lists_sets_each_corner() {
    let r = boot(
        "component A\n  state xs = [1, 2, 3]\n  view\n    column\n      for x, i in xs key=x\n        Cell(first=(i == 0), last=(i == xs.length - 1), n=x)\n\ncomponent Cell\n  props\n    first: bool\n    last: bool\n    n: number\n  state z = 0\n  action noop\n    z = 1\n  view\n    scroll testId=`b${n}` swipeContent=`row-${n}` swipeTrailing=`del-${n}` overflow-x=\"scroll\" overflow-y=\"hidden\" height=50 display=\"flex\" flex-direction=\"row\" border-radius=(first and last ? \"12px\" : first ? \"12px 12px 2px 2px\" : last ? \"2px 2px 12px 12px\" : \"2px\")\n      row id=`row-${n}` width=\"100%\" flex-shrink=0\n        text \"x\"\n      button id=`del-${n}` width=40 press=noop\n        text \"d\"\n",
    );
    let corners = |id: &str| {
        let s = style_of(&r, id);
        [
            s.border_radius_top_left,
            s.border_radius_top_right,
            s.border_radius_bottom_right,
            s.border_radius_bottom_left,
        ]
        .map(|d| format!("{d:?}"))
    };
    let p = |n: f32| format!("{:?}", exact_kernel::Dimension::Points(n));
    assert_eq!(corners("b1"), [p(12.0), p(12.0), p(2.0), p(2.0)]);
    assert_eq!(corners("b2"), [p(2.0), p(2.0), p(2.0), p(2.0)]);
    assert_eq!(corners("b3"), [p(2.0), p(2.0), p(12.0), p(12.0)]);
}

/// CSS's two-value shorthands are split as the box shorthands are: `gap`
/// into row and column, `overflow` and `overscroll-behavior` into x and y,
/// from a literal or a literal arm; a computed list is refused naming the
/// longhands, and a third value is refused.
#[test]
fn two_value_shorthands_split_into_their_rows() {
    for value in ["\"8px 16px\"", "(on ? \"8px 16px\" : \"4px\")"] {
        let r = boot(&format!(
            "component A\n  state on = true\n  view\n    column\n      box testId=\"x\" width=10 height=10 gap={value} overflow=(on ? \"hidden auto\" : \"visible\") overscroll-behavior=\"contain none\"\n"
        ));
        let s = style_of(&r, "x");
        assert_eq!((s.row_gap, s.column_gap), (8.0, 16.0), "{value}");
        assert_eq!(
            format!("{:?} {:?}", s.overflow_x, s.overflow_y),
            "Hidden Auto"
        );
        assert_eq!(
            format!(
                "{:?} {:?}",
                s.overscroll_behavior_x, s.overscroll_behavior_y
            ),
            "Contain None"
        );
    }
    for (attr, says) in [
        ("gap=`${g}px 4px`", "write `row-gap`/`column-gap`"),
        (
            "overflow=(on ? `hidden ${o}` : \"auto\")",
            "write `overflow-x`/`overflow-y`",
        ),
        (
            "overscroll-behavior=\"none \" + o",
            "write `overscroll-behavior-x`/`overscroll-behavior-y`",
        ),
    ] {
        let e = contract::compile(&format!(
            "component A\n  state g = 8\n  state o = \"auto\"\n  state on = false\n  view\n    box {attr}\n"
        ))
        .unwrap_err();
        assert_eq!(e.id, "lower-css-shorthand", "{attr}: {e}");
        assert!(e.message.contains(says), "{attr}: {e}");
    }
    let e = contract::compile("component A\n  view\n    box gap=\"1px 2px 3px\"\n").unwrap_err();
    assert_eq!(e.id, "lower-attr-value");
    assert!(e.message.contains("one or two values (row, column)"), "{e}");
}

/// LLP 1053.000.000: a glass group on a box that scrolls is refused, the
/// box's overflow computed as CSS Overflow 3 §3.1 does (a `visible` axis
/// beside a non-`visible` one is `auto`), from every row and each literal
/// arm of a choice (the review of the two-value split).
#[test]
fn a_glass_group_reads_every_overflow_row_and_arm() {
    for overflow in [
        "overflow=\"hidden\"",
        "overflow=\"visible\"",
        // `""` and `"unset"` declare nothing: both axes stay `visible`.
        "overflow=\"\"",
        "overflow=\"unset\"",
        // Each arm keeps its two axes together: neither scrolls.
        "overflow=(on ? \"hidden\" : \"visible\")",
        // A computed x beside a `visible` y could compute anything; doubt is no refusal.
        "overflow-x=o",
    ] {
        contract::compile(&format!(
            "component App\n  state on = true\n  state o = \"hidden\"\n  view\n    box glassGroup=0 {overflow}\n"
        ))
        .unwrap_or_else(|e| panic!("{overflow}: {e}"));
    }
    // A later literal overrides an earlier computed row; an arm that scrolls,
    // or a known `hidden` beside a computed axis, may scroll.
    for overflow in [
        "overflow=\"hidden visible\"",
        "overflow-x=\"hidden\"",
        "overflow-x=o overflow=\"auto\"",
        "overflow=(on ? \"hidden visible\" : \"hidden\")",
        "overflow-x=\"hidden\" overflow-y=o",
    ] {
        let e = contract::compile(&format!(
            "component App\n  state on = true\n  state o = \"hidden\"\n  view\n    box glassGroup=4 height=40 {overflow}\n"
        ))
        .unwrap_err();
        assert_eq!(e.id, "lower-glass-group", "{overflow}: {e}");
    }
}
