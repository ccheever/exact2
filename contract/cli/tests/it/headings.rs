//! LLP 1115 D3: a heading is the platform's text style for its level (1
//! `title1`, 2 `title2`, 3 `title3`, 4 and on `headline`), read from the
//! schema's ramp at the root font size (the platform's body size), so it
//! follows Dynamic Type as Apple's own ramp does. Native hosts keep the
//! text style's weight; web, neutral and terminal looks emphasize titles.
//! An authored `font`, `font-size` or `font-weight` wins over its defaults.
use exact_kernel::{Kernel, RowValue, StyleId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SOURCE: &str = r#"component App
  view
    column font-size=30 font-weight=300
      text testId="h1" role="heading" aria-level=1 "One"
      text testId="h2" role="heading" "Two"
      text testId="h3" role="heading" aria-level=3 "Three"
      text testId="h5" role="heading" aria-level=5 "Five"
      text testId="chosen" role="heading" aria-level=(1 > 2 ? 1 : 3) "Chosen"
      text testId="sized" role="heading" aria-level=1 font-size=20 "Sized"
      text testId="weighted" role="heading" aria-level=1 font-weight=800 "Weighted"
      text testId="plain" "Plain"
      text testId="caption" font="-exact-caption1" "Caption"
      text testId="footnote" font="-apple-system-footnote" font-weight=500 "Footnote"
"#;

fn number(r: &Runner<NoData>, id: &str, row: StyleId) -> f64 {
    let key = r.kernel().find_by_test_id(id)[0];
    match r.kernel().node_by_key(key).unwrap().computed(row) {
        RowValue::Number(n) => n,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_heading_is_its_levels_text_style_at_the_root_size_unless_written() {
    let plan = contract::compile(SOURCE).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let size = |r: &Runner<NoData>, id| number(r, id, StyleId::FontSize);
    let weight = |r: &Runner<NoData>, id| number(r, id, StyleId::FontWeight);
    // iOS at the default Dynamic Type: body 17.
    assert!(r.set_root_font_size(17.0).unwrap().is_some());
    assert_eq!(size(&r, "h1"), 28.0);
    assert_eq!(size(&r, "h2"), 22.0, "ARIA's default level is 2");
    // And said, so a host that reads the level (iOS's header-shaped route) sees it.
    let level = exact_kernel::PropId::from_name("accessibilityHeadingLevel").unwrap();
    let node = |id| {
        r.kernel()
            .node_by_key(r.kernel().find_by_test_id(id)[0])
            .unwrap()
    };
    assert_eq!(
        node("h2").props.get(level),
        Some(&exact_kernel::PropValue::Int(2))
    );
    assert_eq!(
        node("h3").props.get(level),
        Some(&exact_kernel::PropValue::Int(3))
    );
    assert_eq!(size(&r, "h3"), 20.0);
    assert_eq!(size(&r, "h5"), 17.0);
    assert_eq!(size(&r, "chosen"), 20.0, "a choice of levels, of styles");
    assert_eq!(
        weight(&r, "h1"),
        700.0,
        "a title's emphasized weight, not the parent's"
    );
    assert_eq!(weight(&r, "h2"), 700.0);
    assert_eq!(
        weight(&r, "h3"),
        600.0,
        "Apple's emphasized Title 3 is semibold"
    );
    assert_eq!(weight(&r, "chosen"), 600.0, "the chosen level's weight");
    assert_eq!(weight(&r, "h5"), 600.0, "headline is semibold");
    assert_eq!(size(&r, "sized"), 20.0, "the author's size wins");
    assert_eq!(weight(&r, "sized"), 700.0);
    assert_eq!(size(&r, "weighted"), 28.0);
    assert_eq!(weight(&r, "weighted"), 800.0, "the author's weight wins");
    assert_eq!(size(&r, "plain"), 30.0, "a non-heading inherits");
    assert_eq!(size(&r, "caption"), 12.0);
    assert_eq!(weight(&r, "caption"), 400.0);
    assert_eq!(size(&r, "footnote"), 13.0);
    assert_eq!(
        weight(&r, "footnote"),
        500.0,
        "a longhand after `font` wins"
    );
    // AX5: Apple's ramp, not a ratio of the body size.
    assert!(r.set_root_font_size(53.0).unwrap().is_some());
    assert_eq!(size(&r, "h1"), 58.0);
    assert_eq!(size(&r, "h5"), 53.0);
    // The Mac's body, 13.
    assert!(r.set_root_font_size(13.0).unwrap().is_some());
    assert_eq!(size(&r, "h1"), 22.0);
    assert_eq!(size(&r, "h2"), 17.0);
    assert_eq!(size(&r, "h3"), 15.0);
}

// @ref LLP 1116.007 D3.2 — round 9's builders wrote the role's number after a hyphen.
#[test]
fn a_text_styles_number_may_follow_a_hyphen() {
    let mut src = String::from("component App\n  view\n    column\n");
    for (i, name) in ["title1", "title2", "title3", "caption1", "caption2"]
        .iter()
        .enumerate()
    {
        let (head, number) = name.split_at(name.len() - 1);
        src += &format!(
            "      text testId=\"a{i}\" font=\"-exact-{name}\" \"x\"\n      text testId=\"b{i}\" font=\"-exact-{head}-{number}\" \"x\"\n      text testId=\"c{i}\" font-size=\"-exact-{head}-{number}\" \"x\"\n"
        );
    }
    let plan = contract::compile(&src).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(r.set_root_font_size(17.0).unwrap().is_some());
    for (i, size) in [28.0, 22.0, 20.0, 12.0, 11.0].into_iter().enumerate() {
        for prefix in ["a", "b", "c"] {
            let id = format!("{prefix}{i}");
            assert_eq!(number(&r, &id, StyleId::FontSize), size, "{id}");
        }
        assert_eq!(
            number(&r, &format!("b{i}"), StyleId::FontWeight),
            number(&r, &format!("a{i}"), StyleId::FontWeight),
            "b{i}"
        );
    }
}

#[test]
fn font_takes_only_a_text_style() {
    for value in [
        "\"bold 16px Helvetica\"",
        "16",
        "\"-exact-title9\"",
        "\"-exact-title-4\"",
    ] {
        let src = format!("component App\n  view\n    text font={value} \"x\"\n");
        let e = contract::compile(&src).expect_err(value).to_string();
        assert!(
            e.contains("`font` takes a platform text style"),
            "{value}: {e}"
        );
    }
}

/// `font="inherit"`, the common reset's: every font longhand is a written
/// clear, which takes the surrounding text's value past a platform control's
/// start, as `color="inherit"` does.
#[test]
fn font_inherit_clears_every_font_longhand() {
    let src = "component App\n  view\n    column font-size=20 font-weight=600\n      button testId=\"reset\" appearance=\"none\" font=\"inherit\" \"Sort\"\n";
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let node = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("reset")[0])
        .unwrap();
    for row in [
        StyleId::FontStyle,
        StyleId::FontWeight,
        StyleId::FontSize,
        StyleId::LineHeight,
        StyleId::FontFamily,
    ] {
        assert!(
            node.style.written.has(row) && !node.style.mask.has(row),
            "{row:?}"
        );
        assert!(!node.starts(row), "{row:?}");
    }
    assert_eq!(number(&r, "reset", StyleId::FontSize), 20.0);
    assert_eq!(number(&r, "reset", StyleId::FontWeight), 600.0);
    // The longhand alone, as a reset also writes it.
    let src = "component App\n  view\n    column font-family=\"Georgia\"\n      button testId=\"field\" font-family=\"inherit\" \"Sort\"\n";
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let node = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("field")[0])
        .unwrap();
    assert!(
        node.style.written.has(StyleId::FontFamily) && !node.style.mask.has(StyleId::FontFamily)
    );
}

/// Web headings retain their emphasized weights; an authored weight wins
/// at every level.
#[test]
fn each_heading_level_has_its_emphasized_weight() {
    let mut src = String::from("component App\n  view\n    column font-weight=300\n");
    for level in 1..=6 {
        src += &format!("      text testId=\"h{level}\" role=\"heading\" aria-level={level} \"x\"\n      text testId=\"w{level}\" role=\"heading\" aria-level={level} font-weight=400 \"x\"\n");
    }
    let plan = contract::compile(&src).unwrap_or_else(|e| panic!("{e}"));
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let weights: Vec<f64> = (1..=6)
        .map(|l| number(&r, &format!("h{l}"), StyleId::FontWeight))
        .collect();
    assert_eq!(weights, [700.0, 700.0, 600.0, 600.0, 600.0, 600.0]);
    let written: Vec<f64> = (1..=6)
        .map(|l| number(&r, &format!("w{l}"), StyleId::FontWeight))
        .collect();
    assert_eq!(written, [400.0; 6], "a written weight wins");
}
