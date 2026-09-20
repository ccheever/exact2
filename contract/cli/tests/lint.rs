//! LLP 1017 P1: what the compiler refuses about values and names, and what
//! bake's layout lint refuses that the compiler cannot see.

use contract::BakeError;
use exact_plan::Value;
use exact_runner::{DataError, DataSource};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn an_old_spelling_is_refused_with_the_css_name() {
    let src = "component A\n  view\n    text \"a\" size=13\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "lower-unknown-attr");
    assert!(e.message.contains("`font-size`"), "{e}");
    let src = "component A\n  state n = 0\n  action go writes n\n    n = 1\n  view\n    button press=go label=\"Go\"\n      text \"a\"\n";
    let e = contract::compile(src).unwrap_err();
    assert!(e.message.contains("`aria-label`"), "{e}");
}

#[test]
fn a_literal_value_is_checked_against_its_row_at_compile_time() {
    for (src, id, needle) in [
        (
            "component A\n  view\n    column width=true\n      text \"a\"\n",
            "lower-attr-value",
            "not a bool",
        ),
        (
            "component A\n  view\n    column align-items=\"middle\"\n      text \"a\"\n",
            "lower-attr-value",
            "align_items",
        ),
        (
            "component A\n  view\n    column background-color=\"red\"\n      text \"a\"\n",
            "lower-attr-value",
            "#rrggbb",
        ),
        (
            "component A\n  view\n    column padding=\"auto\"\n      text \"a\"\n",
            "lower-attr-value",
            "auto",
        ),
        (
            "component A\n  state on = true\n  view\n    column width=on\n      text \"a\"\n",
            "lower-attr-type",
            "bool",
        ),
        (
            "component A\n  view\n    column disabled=1\n      text \"a\"\n",
            "lower-attr-type",
            "a bool",
        ),
    ] {
        let e = contract::compile(src).unwrap_err();
        assert_eq!(e.id, id, "{src}: {e}");
        assert!(e.message.contains(needle), "{src}: {e}");
    }
    // What passes: numbers, percentages, `auto`, hex colours, enum names,
    // and a computed number or string.
    let src = "component A\n  state w = 10\n  state c = \"#fff\"\n  view\n    column width=w height=\"50%\" max-width=\"auto\" background-color=c align-items=\"center\" flex=1\n      text \"a\" font-size=14 color=\"#00000080\"\n";
    contract::compile(src).unwrap();
}

#[test]
fn a_hyphenated_unknown_name_says_why() {
    let src =
        "component A\n  state a = 1\n  state b = 2\n  derive c = a-b\n  view\n    text `${c}`\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-unknown-name");
    assert!(e.message.contains("`a - b`"), "{e}");
    let src =
        "component A\n  state a = 1\n  state b = 2\n  derive c = a - b\n  view\n    text `${c}`\n";
    contract::compile(src).unwrap();
}

#[test]
fn bake_refuses_a_scroll_that_only_a_row_could_have_bounded_and_did_not() {
    // The compiler's rule lets this through (the parent is a `row`, whose
    // stretch could bound it); at the first frame the row is content-sized,
    // so the scroll is exactly as tall as its children.
    let src = "component A\n  view\n    row testId=\"root\"\n      scroll testId=\"rows\"\n        column\n          text \"one\"\n          text \"two\"\n          text \"three\"\n";
    let plan = contract::compile(src).unwrap();
    let e = contract::bake(plan, NoData).unwrap_err();
    match e {
        BakeError::Lint { id, message } => {
            assert_eq!(id, "bake-scroll-unbounded");
            assert!(message.contains("testId=\"rows\""), "{message}");
        }
        other => panic!("{other:?}"),
    }
    // Bounded by the row: fine.
    let src = "component A\n  view\n    row height=40 testId=\"root\"\n      scroll testId=\"rows\"\n        column\n          text \"one\"\n          text \"two\"\n          text \"three\"\n";
    let plan = contract::compile(src).unwrap();
    contract::bake(plan, NoData).unwrap();
}

#[test]
fn a_horizontal_scroll_can_grow_vertically_with_its_contents() {
    let src = "component A\n  view\n    column width=200\n      scroll width=200 overflow-x=\"scroll\" overflow-y=\"hidden\" scroll-snap-type=\"x mandatory\"\n        row width=400\n          box width=200 height=80 scroll-snap-align=\"start\"\n          box width=200 height=80\n";
    contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
}

#[test]
fn bake_refuses_a_pressable_with_zero_area() {
    let src = "component A\n  state n = 0\n  action go writes n\n    n = 1\n  view\n    column\n      button press=go width=0 height=0 testId=\"go\"\n";
    let plan = contract::compile(src).unwrap();
    let e = contract::bake(plan, NoData).unwrap_err();
    match e {
        BakeError::Lint { id, message } => {
            assert_eq!(id, "bake-zero-size");
            assert!(message.contains("testId=\"go\""), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn conditional_style_literals_are_refused_at_the_offending_branch() {
    for (property, value, bad) in [
        ("top", r#"(on ? "0px" : "0%")"#, "0px"),
        ("top", r#"(on ? "0%" : "0px")"#, "0px"),
        (
            "align-items",
            r#"(on ? "center" : (on ? "flex-end" : "middle"))"#,
            "middle",
        ),
        (
            "padding",
            r#"(match maybe { case some(n) => n, case none => "auto" })"#,
            "auto",
        ),
        (
            "top",
            r#"(match maybe { case some(n) => "0px", case none => n })"#,
            "0px",
        ),
    ] {
        let source = format!("component App\n  state on = false\n  state n = \"0%\"\n  state maybe = some(\"10%\")\n  view\n    column {property}={value}\n      text \"branch\"\n");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{source}: {error}");
        assert!(error.message.contains(bad), "{error}");
        let line = source.lines().nth(5).unwrap();
        assert_eq!(
            error.span,
            (6, (line.find(&format!("\"{bad}\"")).unwrap() + 1) as u32),
            "{error}"
        );
    }
}

#[test]
fn conditional_style_checks_preserve_computation_and_match_bindings() {
    let source = r#"component App
  state on = false
  state n = true
  state maybe = some(20)
  action toggle writes on
    on = !on
  view
    column testId="branch" top=(on ? -10 : (match maybe { case some(n) => n, case none => 0 })) align-items=(on ? "center" : "flex-end")
      text "branch"
"#;
    let plan = contract::compile(source).unwrap();
    let mut runner = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let top = |runner: &exact_runner::Runner<NoData>| {
        let kernel = runner.kernel();
        let key = kernel.find_by_test_id("branch")[0];
        kernel.node_by_key(key).unwrap().style.top
    };
    assert_eq!(top(&runner), exact_kernel::Dimension::Points(20.0));
    runner.act("toggle", vec![]).unwrap();
    assert_eq!(top(&runner), exact_kernel::Dimension::Points(-10.0));
    assert!(!runner.is_poisoned());
}
