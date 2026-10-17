//! LLP 1115.000 D12.2: a text style carries its weight and its leading, per
//! platform, when `font-weight` and `line-height` are unsaid. Named through
//! the `font` shorthand (or a heading) the style resets both, beating an
//! inherited value; named by a bare `font-size` it fills only what nothing
//! above says. Per longhand: written on the node, a shorthand's style,
//! inherited, a bare style's, CSS's default. The Mac's `headline` is bold
//! and its styles carry their own leading. One test, its own process: the
//! column is the process's.
use exact_kernel::style::metrics::{set_column, Column};
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
    column testId="root"
      column font-weight=700 line-height=2
        text testId="shorthand" font="-exact-body" "Shorthand"
        text testId="bare" font-size="-exact-headline" "Bare"
        text testId="written" font="-exact-body" font-weight=300 "Written"
      text testId="alone" font-size="-exact-headline" "Alone"
      column testId="parent" font="-exact-headline"
        text testId="child" "Child"
      text testId="h5" role="heading" aria-level=5 "Five"
      text testId="h1" role="heading" aria-level=1 "One"
"#;

fn boot() -> Runner<NoData> {
    let plan = contract::compile(SOURCE).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_root_font_size(13.0).unwrap();
    r
}

fn row<'a>(r: &'a Runner<NoData>, id: &str, row: StyleId) -> RowValue<'a> {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel().node_by_key(key).unwrap().computed(row)
}

fn weight(r: &Runner<NoData>, id: &str) -> f64 {
    match row(r, id, StyleId::FontWeight) {
        RowValue::Number(n) => n,
        other => panic!("{id}: {other:?}"),
    }
}

fn leading(r: &Runner<NoData>, id: &str) -> String {
    format!("{:?}", row(r, id, StyleId::LineHeight))
}

fn view(r: &Runner<NoData>, id: &str) -> u32 {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel().node_by_key(key).unwrap().id
}

#[test]
fn a_text_style_gives_its_weight_and_leading_by_its_precedence_and_column() {
    // The web's column: iOS's weights, `normal` leading.
    set_column(Column::Web);
    let r = boot();
    assert_eq!(
        weight(&r, "shorthand"),
        400.0,
        "a shorthand's style beats an inherited 700"
    );
    assert_eq!(
        leading(&r, "shorthand"),
        leading(&r, "alone"),
        "and an inherited line height: `normal`"
    );
    assert_eq!(
        weight(&r, "bare"),
        700.0,
        "a bare style keeps the inherited weight"
    );
    assert_ne!(
        leading(&r, "bare"),
        leading(&r, "alone"),
        "and the inherited line height"
    );
    assert_eq!(weight(&r, "written"), 300.0, "what the node writes wins");
    assert_eq!(
        weight(&r, "alone"),
        600.0,
        "a bare style fills what nothing above says"
    );
    assert_eq!(
        weight(&r, "child"),
        600.0,
        "a child inherits its parent's style"
    );
    assert_eq!(weight(&r, "h5"), 600.0, "headline is semibold here");
    assert_eq!(
        weight(&r, "h1"),
        700.0,
        "a title's heading keeps the heading's bold"
    );
    // Where each came from.
    let read = |id: &str| exact_runner::agent::node(&r, view(&r, id));
    let shorthand = read("shorthand");
    let me = view(&r, "shorthand");
    assert!(
        shorthand.contains(&format!(
            "\"font_weight\":{{\"value\":400,\"source\":\"style\",\"from\":{me}}}"
        )),
        "{shorthand}"
    );
    let parent = view(&r, "parent");
    let child = read("child");
    assert!(
        child.contains(&format!(
            "\"font_weight\":{{\"value\":600,\"source\":\"style\",\"from\":{parent}}}"
        )),
        "{child}"
    );
    let bare = read("bare");
    assert!(
        bare.contains("\"font_weight\":{\"value\":700,\"source\":\"inherited\""),
        "{bare}"
    );
    let alone = read("alone");
    let me = view(&r, "alone");
    assert!(
        alone.contains(&format!(
            "\"font_weight\":{{\"value\":600,\"source\":\"style\",\"from\":{me}}}"
        )),
        "{alone}"
    );
    let h1 = read("h1");
    assert!(
        h1.contains("\"font_weight\":{\"value\":700,\"source\":\"heading\"}"),
        "{h1}"
    );
    // The Mac's column: a bold headline, and the styles' own leading.
    set_column(Column::Mac);
    let r = boot();
    assert_eq!(weight(&r, "h5"), 700.0, "the Mac's headline is bold");
    assert_eq!(weight(&r, "alone"), 700.0);
    assert_eq!(weight(&r, "shorthand"), 400.0);
    assert!(
        leading(&r, "shorthand").contains("1.2308"),
        "body's 13/16 on the Mac: {}",
        leading(&r, "shorthand")
    );
    set_column(Column::Web);
}
