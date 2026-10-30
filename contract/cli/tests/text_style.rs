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
  state big = false
  action flip
    big = ! big
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
      text testId="h2" role="heading" aria-level=2 "Two"
      text testId="h3" role="heading" aria-level=3 "Three"
      column font-weight=900
        text testId="h-inherited" role="heading" aria-level=1 "Inherited"
      text testId="h-authored" role="heading" aria-level=1 font-size=19 font-weight=500 "Authored"
      text testId="h-bound" role="heading" aria-level=(big ? 3 : 1) "Bound"
      text testId="h-font" role="heading" aria-level=2 font="-exact-headline" "Authored font"
      text testId="h-font-bound" role="heading" aria-level=1 font=(big ? none : "-exact-headline") "Bound font"
      text testId="h-font-authored" role="heading" aria-level=1 font=(big ? "-exact-subheadline" : "-exact-headline") font-weight=500 "Bound authored weight"
      text testId="h-level-authored" role="heading" aria-level=(big ? 3 : 1) font-size=19 font-weight=500 "Bound authored size and weight"
      text testId="h-level-font" role="heading" aria-level=(big ? 3 : 1) font="-exact-headline" "Bound level with font"
      text testId="h-font-shadow" role="heading" aria-level=(big ? 3 : 1) font=(match some("-exact-headline") { case some(big) => none, case none => "-exact-headline" }) "Shadowed font"
      column testId="barehead" font-size="-exact-headline"
        text testId="small" font-size=12 "Small"
      text testId="cleared" font-size="-exact-headline" font-weight="inherit" "Cleared"
      input testId="field" font="-exact-headline" value="x"
      column testId="flipper" font=(big ? "-exact-headline" : "-exact-body")
        text testId="inner" font-size=12 "Inner"
      button testId="flip" press=flip "Flip"
      text testId="para" font-weight=700 line-height=2
        text font="-exact-body" "Body "
        text font-size="-exact-headline" "Bare"
      column font-size="-exact-headline"
        text testId="inheriting" font-size=12 font-weight="inherit" "Inheriting"
      text testId="nested" font-size="-exact-headline"
        text font-size="-exact-body" "Inner"
      column font-size="-exact-headline"
        text testId="passed" font-size="-exact-body" font-weight="inherit" "Passed"
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
    assert_eq!(
        weight(&r, "small"),
        600.0,
        "a bare style's weight is inherited as CSS's is, a child's own size or not"
    );
    assert_eq!(
        weight(&r, "cleared"),
        400.0,
        "a written clear inherits: nothing above, CSS's 400"
    );
    assert_eq!(
        weight(&r, "field"),
        600.0,
        "a style the author names beats a control's own type"
    );
    let h1 = read("h1");
    assert!(
        h1.contains("\"font_weight\":{\"value\":700,\"source\":\"heading\"}"),
        "{h1}"
    );
    // Runs inside a paragraph take their style as a node does: a shorthand's
    // beats the paragraph's 700 and line height 2; a bare one keeps them.
    let runs = {
        let key = r.kernel().find_by_test_id("para")[0];
        let node = r.kernel().node_by_key(key).unwrap();
        node.text_runs()
            .iter()
            .map(|run| (run.style.font_weight, run.style.line_height))
            .collect::<Vec<_>>()
    };
    assert_eq!(runs[0].0, 400, "{runs:?}");
    assert_eq!(runs[1].0, 700, "{runs:?}");
    assert_ne!(
        runs[0].1, runs[1].1,
        "the shorthand's `normal` against the paragraph's 2: {runs:?}"
    );
    assert_eq!(
        weight(&r, "inheriting"),
        600.0,
        "a clear below a bare style inherits the style's"
    );
    let inner = {
        let key = r.kernel().find_by_test_id("nested")[0];
        r.kernel().node_by_key(key).unwrap().text_runs()[0]
            .style
            .font_weight
    };
    assert_eq!(
        inner, 400,
        "a nearer bare style's run takes its own, not the outer's fill"
    );
    assert_eq!(
        weight(&r, "passed"),
        600.0,
        "a nearer bare style that clears the row passes it to the further one's"
    );
    // A bound shorthand moving its style moves what its children inherit,
    // a child with a size of its own included.
    assert_eq!(weight(&r, "inner"), 400.0);
    let mut r = r;
    let flip = view(&r, "flip");
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    assert_eq!(
        weight(&r, "inner"),
        600.0,
        "the parent's style is headline now"
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
    for column in [
        Column::Ios,
        Column::Mac,
        Column::Material,
        Column::Web,
        Column::Neutral,
        Column::Terminal,
    ] {
        set_column(column);
        let mut r = boot();
        let native = matches!(column, Column::Ios | Column::Mac | Column::Material);
        let title = if native { 400.0 } else { 700.0 };
        let third = if native { 400.0 } else { 600.0 };
        for id in ["h1", "h2", "h-inherited", "h-bound", "h-font-shadow"] {
            assert_eq!(weight(&r, id), title, "{column:?} {id}");
        }
        assert_eq!(weight(&r, "h3"), third, "{column:?} title3");
        assert_eq!(
            weight(&r, "h-authored"),
            500.0,
            "{column:?} authored weight wins"
        );
        assert_eq!(
            row(&r, "h-authored", StyleId::FontSize),
            RowValue::Number(19.0)
        );
        let headline = if column == Column::Mac { 700.0 } else { 600.0 };
        assert_eq!(weight(&r, "h5"), headline);
        for id in ["h-font-authored", "h-level-authored"] {
            assert_eq!(
                weight(&r, id),
                500.0,
                "{column:?} authored weight initially"
            );
        }
        assert_eq!(
            weight(&r, "h-font"),
            headline,
            "{column:?} authored font wins"
        );
        assert_eq!(
            weight(&r, "h-font-bound"),
            headline,
            "{column:?} bound font wins"
        );
        let flip = view(&r, "flip");
        r.dispatch(flip, exact_runner::Event::Press).unwrap();
        assert_eq!(weight(&r, "h-bound"), third, "{column:?} changed level");
        for id in ["h-font-authored", "h-level-authored"] {
            assert_eq!(
                weight(&r, id),
                500.0,
                "{column:?} a changed default does not replace authored weight"
            );
        }
        assert_eq!(
            row(&r, "h-level-authored", StyleId::FontSize),
            RowValue::Number(19.0)
        );
        assert_eq!(
            row(&r, "h-level-font", StyleId::FontSize),
            row(&r, "alone", StyleId::FontSize)
        );
        assert_eq!(weight(&r, "h-level-font"), headline);
        assert_eq!(
            weight(&r, "h-font-shadow"),
            third,
            "{column:?} font bindings do not capture the level"
        );
        assert_eq!(
            weight(&r, "h-font-bound"),
            title,
            "{column:?} removing a font restores the heading weight"
        );
    }
    set_column(Column::Web);
}
