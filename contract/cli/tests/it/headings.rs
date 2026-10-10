//! LLP 1115 D3: a heading is the platform's text style for its level (1
//! `title1`, 2 `title2`, 3 `title3`, 4 and on `headline`), read from the
//! schema's ramp at the root font size (the platform's body size), so it
//! follows Dynamic Type as Apple's own ramp does. A written `font-size` or
//! `font-weight` wins; an inherited one does not. `font` takes a text style
//! as WebKit's `font: -apple-system-headline` does.
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
    column font-size=30 font-weight=700
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
    assert_eq!(size(&r, "h3"), 20.0);
    assert_eq!(size(&r, "h5"), 17.0);
    assert_eq!(size(&r, "chosen"), 20.0, "a choice of levels, of styles");
    assert_eq!(
        weight(&r, "h1"),
        400.0,
        "a title's weight, not the parent's"
    );
    assert_eq!(weight(&r, "h5"), 600.0, "headline is semibold");
    assert_eq!(size(&r, "sized"), 20.0, "the author's size wins");
    assert_eq!(weight(&r, "sized"), 400.0);
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

#[test]
fn font_takes_only_a_text_style() {
    for value in ["\"bold 16px Helvetica\"", "16", "\"-exact-title9\""] {
        let src = format!("component App\n  view\n    text font={value} \"x\"\n");
        let e = contract::compile(&src).expect_err(value).to_string();
        assert!(
            e.contains("`font` takes a platform text style"),
            "{value}: {e}"
        );
    }
}
